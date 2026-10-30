//! Optional Harpoon V §5.4.2.1 bearing ambiguity and towed-array stabilization.
//!
//! This module is deliberately pure. The detection/FCS owners pass an already
//! identified fitted array, observed maneuver, and reacquisition event; this
//! code never reads referee truth, generates a contact, or rolls dice.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

const RULE_ID: &str = "HV-5.4.2.1-TA-OPTIONAL-2026";

fn source() -> &'static serde_json::Value {
    static SOURCE: OnceLock<serde_json::Value> = OnceLock::new();
    SOURCE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/towed-array-optional-2026.json"
        ))
        .expect("verified towed-array optional rule data")
    })
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Configuration {
    /// This is an optional rule. False leaves existing sonar/FCS behavior alone.
    pub enabled: bool,
    /// Exact fitted array keys, conventionally `<platform-class>:<sensor-id>`.
    pub arrays: BTreeMap<String, ArrayCapability>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArrayCapability {
    pub kind: ArrayKind,
    /// Annex sonar generation, not combat-system generation.
    pub generation: u8,
    /// K1 category. Physical length in meters is not supplied by the source.
    pub length_category: Option<ArrayLengthCategory>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArrayKind {
    PassiveTowed,
    ActiveTowed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArrayLengthCategory {
    Short,
    Long,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Stable caller-owned track/contact key -> optional-rule bearing state.
    pub tracks: BTreeMap<String, TrackState>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrackState {
    pub array_key: String,
    pub phase: BearingPhase,
    /// Track existence is distinct from whether its bearing data is usable.
    pub track_established: bool,
    pub detected_at_tick: i64,
    pub track_established_at_tick: Option<i64>,
    pub source_rule: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum BearingPhase {
    BearingAmbiguous,
    Maneuvering {
        maneuver_started_at_tick: i64,
        course_change_millidegrees: i32,
    },
    AwaitingReacquisition {
        maneuver_completed_at_tick: i64,
    },
    Stabilizing {
        ready_at_tick: i64,
        turns: u8,
        speed_column: TacticalSpeedColumn,
    },
    /// Reacquired track, but source inputs do not select a stabilization cell.
    StabilizationPending {
        reason: String,
    },
    Stable,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TacticalSpeedColumn {
    Kt5,
    Kt10,
    Kt15Plus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transition {
    pub rule_id: String,
    pub track_key: String,
    pub event: String,
    pub at_tick: i64,
    pub phase: Option<BearingPhase>,
    pub track_established: bool,
    pub bearing_usable: bool,
    pub classification_allowed: bool,
    pub source_pages: Vec<String>,
    pub pending_reason: Option<String>,
}

fn transition(
    key: &str,
    event: &str,
    at: i64,
    track: Option<&TrackState>,
    pending: Option<String>,
) -> Transition {
    let phase = track.map(|t| t.phase.clone());
    let established = track.is_some_and(|t| t.track_established);
    let bearing_usable = track.is_some_and(|t| matches!(&t.phase, BearingPhase::Stable));
    let classification_allowed = track.is_some_and(|t| {
        matches!(
            &t.phase,
            BearingPhase::Stabilizing { .. }
                | BearingPhase::StabilizationPending { .. }
                | BearingPhase::Stable
        )
    });
    Transition {
        rule_id: RULE_ID.to_owned(),
        track_key: key.to_owned(),
        event: event.to_owned(),
        at_tick: at,
        phase,
        track_established: established,
        bearing_usable,
        classification_allowed,
        source_pages: vec!["14-2.HpnV_Rules_1_Jan_26.pdf PDF48 / printed 5-9".into()],
        pending_reason: pending,
    }
}

/// Record a real successful sonar detection. For this optional rule only,
/// passive towed arrays below Gen 7 enter the left/right ambiguity state.
pub fn on_detection(
    config: &Configuration,
    state: &mut State,
    track_key: &str,
    array_key: &str,
    at_tick: i64,
) -> Result<Transition, String> {
    if !config.enabled {
        return Ok(transition(
            track_key,
            "optional_rule_disabled",
            at_tick,
            None,
            None,
        ));
    }
    if at_tick < 0 || track_key.is_empty() {
        return Err("5.4.2.1 requires a nonnegative tick and stable nonempty track key".into());
    }
    let capability = config.arrays.get(array_key).ok_or_else(|| {
        format!("5.4.2.1 exact fitted towed-array capability missing: {array_key}")
    })?;
    if capability.generation == 0 {
        return Err("5.4.2.1 sonar generation must be an explicit positive Annex value".into());
    }
    if let Some(existing) = state.tracks.get(track_key) {
        // Repeated reports do not erase an already resolved track state.
        return Ok(transition(
            track_key,
            "detection_refresh_no_reset",
            at_tick,
            Some(existing),
            None,
        ));
    }
    let ambiguous = capability.kind == ArrayKind::PassiveTowed && capability.generation < 7;
    let phase = if ambiguous {
        BearingPhase::BearingAmbiguous
    } else {
        BearingPhase::Stable
    };
    let track = TrackState {
        array_key: array_key.to_owned(),
        phase,
        track_established: !ambiguous,
        detected_at_tick: at_tick,
        track_established_at_tick: (!ambiguous).then_some(at_tick),
        source_rule: RULE_ID.into(),
    };
    state.tracks.insert(track_key.to_owned(), track);
    Ok(transition(
        track_key,
        if ambiguous {
            "left_right_bearing_ambiguous"
        } else {
            "ambiguity_exception_track_established"
        },
        at_tick,
        state.tracks.get(track_key),
        None,
    ))
}

/// Begin/record the actual 45–135° port or starboard course change. Bearings
/// are unusable while this state is active. The caller supplies observed own-
/// ship course delta; this module does not consult target truth.
pub fn begin_maneuver(
    state: &mut State,
    track_key: &str,
    signed_course_change_deg: f64,
    at_tick: i64,
) -> Result<Transition, String> {
    if !signed_course_change_deg.is_finite() || at_tick < 0 {
        return Err("5.4.2.1 maneuver requires finite course change and nonnegative tick".into());
    }
    let magnitude = signed_course_change_deg.abs();
    if !(45.0..=135.0).contains(&magnitude) {
        return Err(
            "5.4.2.1 ambiguity resolution needs a 45–135 degree port or starboard course change"
                .into(),
        );
    }
    let track = state
        .tracks
        .get_mut(track_key)
        .ok_or("5.4.2.1 track has no recorded array detection")?;
    if !matches!(
        &track.phase,
        BearingPhase::BearingAmbiguous | BearingPhase::Maneuvering { .. }
    ) {
        return Err("5.4.2.1 this track does not require ambiguity-resolution maneuver".into());
    }
    track.phase = BearingPhase::Maneuvering {
        maneuver_started_at_tick: at_tick,
        course_change_millidegrees: (signed_course_change_deg * 1000.0).round() as i32,
    };
    Ok(transition(
        track_key,
        "maneuver_bearing_unusable",
        at_tick,
        Some(track),
        None,
    ))
}

/// Observe realized course relative to the last completed course change.
/// Before first reacquisition, the net change must also be 45–135 degrees;
/// established tracks need only wait for the actual ordered course to finish
/// before beginning the source stabilization interval.
pub fn observe_course_change(
    state: &mut State,
    track_key: &str,
    signed_net_course_change_deg: f64,
    course_change_complete: bool,
    at_tick: i64,
) -> Result<Transition, String> {
    if !signed_net_course_change_deg.is_finite() || at_tick < 0 {
        return Err(
            "5.4.2.1 actual course observation requires finite change and nonnegative tick".into(),
        );
    }
    let track = state
        .tracks
        .get_mut(track_key)
        .ok_or("5.4.2.1 course observation has no detected track")?;
    let delta_millidegrees = (signed_net_course_change_deg * 1000.0).round() as i32;
    match &track.phase {
        BearingPhase::BearingAmbiguous if delta_millidegrees != 0 => {
            let magnitude = signed_net_course_change_deg.abs();
            track.phase = if !track.track_established && course_change_complete {
                if (45.0..=135.0).contains(&magnitude) {
                    BearingPhase::AwaitingReacquisition {
                        maneuver_completed_at_tick: at_tick,
                    }
                } else {
                    // A completed but out-of-band turn does not resolve the
                    // ambiguity; the caller may begin another turn from this
                    // newly completed course.
                    BearingPhase::BearingAmbiguous
                }
            } else {
                BearingPhase::Maneuvering {
                    maneuver_started_at_tick: at_tick,
                    course_change_millidegrees: delta_millidegrees,
                }
            };
        }
        BearingPhase::Stable
        | BearingPhase::Stabilizing { .. }
        | BearingPhase::StabilizationPending { .. }
            if track.track_established && delta_millidegrees != 0 =>
        {
            track.phase = BearingPhase::Maneuvering {
                maneuver_started_at_tick: at_tick,
                course_change_millidegrees: delta_millidegrees,
            };
        }
        BearingPhase::Maneuvering {
            maneuver_started_at_tick,
            ..
        } => {
            let started = *maneuver_started_at_tick;
            if at_tick < started {
                return Err("5.4.2.1 actual course observation precedes maneuver start".into());
            }
            let magnitude = signed_net_course_change_deg.abs();
            if track.track_established && course_change_complete {
                // The World adapter starts stabilization using this array's
                // exact K1 category and actual speed at completed turn.
            } else if !track.track_established && course_change_complete {
                track.phase = if (45.0..=135.0).contains(&magnitude) {
                    BearingPhase::AwaitingReacquisition {
                        maneuver_completed_at_tick: at_tick,
                    }
                } else {
                    BearingPhase::BearingAmbiguous
                };
            } else if !track.track_established && delta_millidegrees == 0 {
                track.phase = BearingPhase::BearingAmbiguous;
            } else {
                track.phase = BearingPhase::Maneuvering {
                    maneuver_started_at_tick: started,
                    course_change_millidegrees: delta_millidegrees,
                };
            }
        }
        _ => {}
    }
    Ok(transition(
        track_key,
        "actual_course_observed",
        at_tick,
        Some(track),
        None,
    ))
}

/// Mark that the course change actually completed. Reacquisition cannot
/// precede this event; the caller owns the authoritative course execution.
pub fn complete_maneuver(
    state: &mut State,
    track_key: &str,
    at_tick: i64,
) -> Result<Transition, String> {
    if at_tick < 0 {
        return Err("5.4.2.1 maneuver completion tick must be nonnegative".into());
    }
    let track = state
        .tracks
        .get_mut(track_key)
        .ok_or("5.4.2.1 maneuver completion has no track state")?;
    let BearingPhase::Maneuvering {
        maneuver_started_at_tick,
        ..
    } = &track.phase
    else {
        return Err("5.4.2.1 no active ambiguity-resolution maneuver to complete".into());
    };
    if at_tick < *maneuver_started_at_tick {
        return Err("5.4.2.1 maneuver completion precedes maneuver start".into());
    }
    track.phase = BearingPhase::AwaitingReacquisition {
        maneuver_completed_at_tick: at_tick,
    };
    Ok(transition(
        track_key,
        "maneuver_completed_reacquisition_required",
        at_tick,
        Some(track),
        None,
    ))
}

/// Record successful reacquisition after maneuver. The printed table has only
/// columns 5, 10, and 15+ kt. Exact 5/10 or >=15 selects a printed column;
/// other speeds remain explicit pending, with no interpolation or guessed band.
pub fn reacquire(
    config: &Configuration,
    state: &mut State,
    track_key: &str,
    speed_kts: f64,
    at_tick: i64,
) -> Result<Transition, String> {
    if !speed_kts.is_finite() || speed_kts < 0.0 || at_tick < 0 {
        return Err("5.4.2.1 reacquisition requires finite nonnegative speed and tick".into());
    }
    let track = state
        .tracks
        .get_mut(track_key)
        .ok_or("5.4.2.1 reacquisition has no track state")?;
    let BearingPhase::AwaitingReacquisition {
        maneuver_completed_at_tick,
    } = &track.phase
    else {
        return Err("5.4.2.1 reacquisition requires explicit maneuver completion".into());
    };
    if at_tick < *maneuver_completed_at_tick {
        return Err("5.4.2.1 reacquisition precedes completion of the course change".into());
    }
    let capability = config.arrays.get(&track.array_key).ok_or_else(|| {
        format!(
            "5.4.2.1 exact fitted towed-array capability missing: {}",
            track.array_key
        )
    })?;
    track.track_established = true;
    track.track_established_at_tick = Some(at_tick);
    let Some(length) = capability.length_category else {
        let reason =
            "Annex K1 Short/Long category unavailable; no stabilization cell can be selected"
                .to_owned();
        track.phase = BearingPhase::StabilizationPending {
            reason: reason.clone(),
        };
        return Ok(transition(
            track_key,
            "reacquired_stabilization_pending",
            at_tick,
            Some(track),
            Some(reason),
        ));
    };
    let speed_column = if (speed_kts - 5.0).abs() < f64::EPSILON {
        Some(TacticalSpeedColumn::Kt5)
    } else if (speed_kts - 10.0).abs() < f64::EPSILON {
        Some(TacticalSpeedColumn::Kt10)
    } else if speed_kts >= 15.0 {
        Some(TacticalSpeedColumn::Kt15Plus)
    } else {
        None
    };
    let Some(column) = speed_column else {
        let reason = format!(
            "source table has 5/10/15+ kt columns and no interpolation/band for {speed_kts:.3} kt"
        );
        track.phase = BearingPhase::StabilizationPending {
            reason: reason.clone(),
        };
        return Ok(transition(
            track_key,
            "reacquired_stabilization_pending",
            at_tick,
            Some(track),
            Some(reason),
        ));
    };
    let turns = stabilization_turns(length, column);
    let turn_seconds = source()["tactical_turn_seconds"]["value"]
        .as_i64()
        .expect("verified tactical turn duration");
    let seconds = i64::from(turns) * turn_seconds;
    let ready_at_tick = at_tick.saturating_add(seconds);
    track.phase = BearingPhase::Stabilizing {
        ready_at_tick,
        turns,
        speed_column: column,
    };
    Ok(transition(
        track_key,
        "reacquired_track_stabilizing",
        at_tick,
        Some(track),
        None,
    ))
}

/// Start/refresh the printed stabilization interval after a later completed
/// course change on an already established towed-array track. This also
/// applies to active and Gen-7 arrays: their exception removes the initial
/// left/right maneuver, not the source's array-kink effect when a ship turns.
pub fn stabilize_after_course_change(
    config: &Configuration,
    state: &mut State,
    track_key: &str,
    speed_kts: f64,
    at_tick: i64,
) -> Result<Transition, String> {
    if !speed_kts.is_finite() || speed_kts < 0.0 || at_tick < 0 {
        return Err(
            "5.4.2.1 course-change stabilization requires finite nonnegative speed and tick".into(),
        );
    }
    let track = state
        .tracks
        .get_mut(track_key)
        .ok_or("5.4.2.1 stabilization has no detected track")?;
    if !track.track_established {
        return Err("5.4.2.1 course-change stabilization requires an established track".into());
    }
    let BearingPhase::Maneuvering {
        maneuver_started_at_tick,
        ..
    } = &track.phase
    else {
        return Err("5.4.2.1 no completed course change is awaiting stabilization".into());
    };
    if at_tick < *maneuver_started_at_tick {
        return Err("5.4.2.1 stabilization precedes course-change completion".into());
    }
    let capability = config.arrays.get(&track.array_key).ok_or_else(|| {
        format!(
            "5.4.2.1 exact fitted towed-array capability missing: {}",
            track.array_key
        )
    })?;
    let Some(length) = capability.length_category else {
        let reason =
            "Annex K1 Short/Long category unavailable; no stabilization cell can be selected"
                .to_owned();
        track.phase = BearingPhase::StabilizationPending {
            reason: reason.clone(),
        };
        return Ok(transition(
            track_key,
            "course_change_stabilization_pending",
            at_tick,
            Some(track),
            Some(reason),
        ));
    };
    let speed_column = if (speed_kts - 5.0).abs() < f64::EPSILON {
        Some(TacticalSpeedColumn::Kt5)
    } else if (speed_kts - 10.0).abs() < f64::EPSILON {
        Some(TacticalSpeedColumn::Kt10)
    } else if speed_kts >= 15.0 {
        Some(TacticalSpeedColumn::Kt15Plus)
    } else {
        None
    };
    let Some(column) = speed_column else {
        let reason = format!(
            "source table has 5/10/15+ kt columns and no interpolation/band for {speed_kts:.3} kt"
        );
        track.phase = BearingPhase::StabilizationPending {
            reason: reason.clone(),
        };
        return Ok(transition(
            track_key,
            "course_change_stabilization_pending",
            at_tick,
            Some(track),
            Some(reason),
        ));
    };
    let turns = stabilization_turns(length, column);
    let turn_seconds = source()["tactical_turn_seconds"]["value"]
        .as_i64()
        .expect("verified tactical turn duration");
    let ready_at_tick = at_tick.saturating_add(i64::from(turns) * turn_seconds);
    track.phase = BearingPhase::Stabilizing {
        ready_at_tick,
        turns,
        speed_column: column,
    };
    Ok(transition(
        track_key,
        "course_change_stabilizing",
        at_tick,
        Some(track),
        None,
    ))
}

fn stabilization_turns(length: ArrayLengthCategory, column: TacticalSpeedColumn) -> u8 {
    let row = match length {
        ArrayLengthCategory::Short => "short",
        ArrayLengthCategory::Long => "long",
    };
    let col = match column {
        TacticalSpeedColumn::Kt5 => 0,
        TacticalSpeedColumn::Kt10 => 1,
        TacticalSpeedColumn::Kt15Plus => 2,
    };
    source()["resolution"]["stabilization"]["turns"][row][col]
        .as_u64()
        .expect("verified stabilization cell") as u8
}

/// Advance the interval after a caller's authoritative simulation tick.
pub fn advance(state: &mut State, track_key: &str, now_tick: i64) -> Result<Transition, String> {
    if now_tick < 0 {
        return Err("5.4.2.1 tick must be nonnegative".into());
    }
    let track = state
        .tracks
        .get_mut(track_key)
        .ok_or("5.4.2.1 unknown track")?;
    if let BearingPhase::Stabilizing { ready_at_tick, .. } = &track.phase {
        if now_tick >= *ready_at_tick {
            track.phase = BearingPhase::Stable;
        }
    }
    Ok(transition(
        track_key,
        "stabilization_clock_advanced",
        now_tick,
        Some(track),
        None,
    ))
}

/// Bearing measurements are unusable during the turn and while stabilization
/// is pending; classification may proceed during a known or unresolved
/// stabilization interval as the source explicitly permits.
pub fn bearing_usable(state: &State, track_key: &str) -> bool {
    state
        .tracks
        .get(track_key)
        .is_some_and(|t| matches!(&t.phase, BearingPhase::Stable))
}

pub fn classification_allowed(state: &State, track_key: &str) -> bool {
    state.tracks.get(track_key).is_some_and(|t| {
        matches!(
            &t.phase,
            BearingPhase::Stabilizing { .. }
                | BearingPhase::StabilizationPending { .. }
                | BearingPhase::Stable
        )
    })
}

/// The source expressly requires an established track before FCS begins.
/// Stabilization can leave that source prerequisite satisfied while bearing
/// data remains unreliable; caller must also require a usable solution input.
pub fn fire_control_track_prerequisite(state: &State, track_key: &str) -> bool {
    state
        .tracks
        .get(track_key)
        .is_some_and(|t| t.track_established)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(
        kind: ArrayKind,
        generation: u8,
        length: Option<ArrayLengthCategory>,
    ) -> Configuration {
        Configuration {
            enabled: true,
            arrays: BTreeMap::from([(
                "platform:array".into(),
                ArrayCapability {
                    kind,
                    generation,
                    length_category: length,
                },
            )]),
        }
    }

    #[test]
    fn passive_gen6_requires_maneuver_then_stabilizes_and_allows_classification() {
        let c = config(ArrayKind::PassiveTowed, 6, Some(ArrayLengthCategory::Long));
        let mut s = State::default();
        let found = on_detection(&c, &mut s, "contact-1", "platform:array", 100).unwrap();
        assert!(!found.track_established && !found.bearing_usable);
        assert!(begin_maneuver(&mut s, "contact-1", 90.0, 101)
            .unwrap()
            .phase
            .is_some());
        complete_maneuver(&mut s, "contact-1", 199).unwrap();
        let reacquired = reacquire(&c, &mut s, "contact-1", 10.0, 200).unwrap();
        assert!(
            reacquired.track_established
                && reacquired.classification_allowed
                && !reacquired.bearing_usable
        );
        assert!(fire_control_track_prerequisite(&s, "contact-1"));
        assert!(!bearing_usable(&s, "contact-1"));
        assert!(!advance(&mut s, "contact-1", 559).unwrap().bearing_usable);
        assert!(advance(&mut s, "contact-1", 560).unwrap().bearing_usable);
    }

    #[test]
    fn active_and_passive_gen7_are_no_maneuver_exceptions() {
        for c in [
            config(ArrayKind::ActiveTowed, 4, Some(ArrayLengthCategory::Short)),
            config(ArrayKind::PassiveTowed, 7, Some(ArrayLengthCategory::Long)),
        ] {
            let mut s = State::default();
            let result = on_detection(&c, &mut s, "contact", "platform:array", 0).unwrap();
            assert!(result.track_established && result.bearing_usable);
            assert!(begin_maneuver(&mut s, "contact", 90.0, 1).is_err());
        }
    }

    #[test]
    fn established_track_turn_restarts_array_stabilization() {
        let c = config(ArrayKind::ActiveTowed, 4, Some(ArrayLengthCategory::Short));
        let mut s = State::default();
        let found = on_detection(&c, &mut s, "contact", "platform:array", 0).unwrap();
        assert!(found.track_established && found.bearing_usable);

        let turning = observe_course_change(&mut s, "contact", 20.0, false, 10).unwrap();
        assert!(matches!(
            turning.phase,
            Some(BearingPhase::Maneuvering { .. })
        ));
        assert!(!turning.bearing_usable);
        let completed = observe_course_change(&mut s, "contact", 60.0, true, 20).unwrap();
        assert!(matches!(
            completed.phase,
            Some(BearingPhase::Maneuvering { .. })
        ));
        let stabilizing = stabilize_after_course_change(&c, &mut s, "contact", 10.0, 20).unwrap();
        assert_eq!(
            stabilizing.phase,
            Some(BearingPhase::Stabilizing {
                ready_at_tick: 200,
                turns: 1,
                speed_column: TacticalSpeedColumn::Kt10,
            })
        );
        assert!(stabilizing.track_established && stabilizing.classification_allowed);
        assert!(!stabilizing.bearing_usable);
        assert!(advance(&mut s, "contact", 199)
            .unwrap()
            .phase
            .as_ref()
            .is_some_and(|p| matches!(p, BearingPhase::Stabilizing { .. })));
        assert!(advance(&mut s, "contact", 200).unwrap().bearing_usable);
    }

    #[test]
    fn actual_turn_blocks_bearing_before_it_reaches_resolution_band() {
        let c = config(ArrayKind::PassiveTowed, 6, Some(ArrayLengthCategory::Long));
        let mut s = State::default();
        on_detection(&c, &mut s, "c", "platform:array", 0).unwrap();
        let turning = observe_course_change(&mut s, "c", 20.0, false, 1).unwrap();
        assert!(matches!(
            turning.phase,
            Some(BearingPhase::Maneuvering { .. })
        ));
        assert!(!turning.bearing_usable && !turning.track_established);
        let not_yet_complete = observe_course_change(&mut s, "c", 60.0, false, 2).unwrap();
        assert!(matches!(
            not_yet_complete.phase,
            Some(BearingPhase::Maneuvering { .. })
        ));
        let completed = observe_course_change(&mut s, "c", 60.0, true, 3).unwrap();
        assert!(matches!(
            completed.phase,
            Some(BearingPhase::AwaitingReacquisition { .. })
        ));
        assert!(!completed.bearing_usable && !completed.track_established);
    }

    #[test]
    fn completed_out_of_band_turn_keeps_ambiguity_and_allows_a_new_course_baseline() {
        let mut s = State::default();
        let c = config(ArrayKind::PassiveTowed, 6, Some(ArrayLengthCategory::Long));
        on_detection(&c, &mut s, "c", "platform:array", 0).unwrap();
        let failed = observe_course_change(&mut s, "c", 150.0, true, 10).unwrap();
        assert_eq!(failed.phase, Some(BearingPhase::BearingAmbiguous));
        assert!(!failed.track_established && !failed.bearing_usable);
        // The World adapter resets its heading baseline at that completed
        // course, so the next 60° is measured independently.
        let eligible = observe_course_change(&mut s, "c", 60.0, true, 20).unwrap();
        assert!(matches!(
            eligible.phase,
            Some(BearingPhase::AwaitingReacquisition { .. })
        ));
    }

    #[test]
    fn unsupported_speed_or_missing_annex_length_stays_pending_without_guess() {
        for (c, speed) in [
            (
                config(ArrayKind::PassiveTowed, 6, Some(ArrayLengthCategory::Short)),
                7.0,
            ),
            (config(ArrayKind::PassiveTowed, 6, None), 10.0),
        ] {
            let mut s = State::default();
            on_detection(&c, &mut s, "c", "platform:array", 0).unwrap();
            begin_maneuver(&mut s, "c", -45.0, 1).unwrap();
            complete_maneuver(&mut s, "c", 10).unwrap();
            let r = reacquire(&c, &mut s, "c", speed, 20).unwrap();
            assert!(matches!(
                r.phase,
                Some(BearingPhase::StabilizationPending { .. })
            ));
            assert!(r.classification_allowed && !r.bearing_usable);
            assert!(!fire_control_track_prerequisite(&s, "missing"));
        }
    }

    #[test]
    fn optional_rule_off_is_no_op_and_bad_maneuver_is_rejected() {
        let mut s = State::default();
        let c = Configuration::default();
        assert_eq!(
            on_detection(&c, &mut s, "c", "missing", 0).unwrap().event,
            "optional_rule_disabled"
        );
        let c = config(ArrayKind::PassiveTowed, 6, Some(ArrayLengthCategory::Short));
        on_detection(&c, &mut s, "c", "platform:array", 0).unwrap();
        assert!(begin_maneuver(&mut s, "c", 44.99, 1).is_err());
    }
}

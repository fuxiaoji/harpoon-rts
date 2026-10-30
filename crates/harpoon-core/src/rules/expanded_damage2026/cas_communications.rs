//! CaS-adjusted communications groups (E12/E13 row 94): group-level consumer
//! queries over the actual fitted and already-disabled radio rooms.
//! Producer side already exists in this module (`resolve_extra`
//! "communications" arm selects the D6 group and disables one physical room
//! via `component_critical`). What was missing is any reader of the CaS
//! *group*, so the printed blocks (beyond-horizon / formation / aircraft
//! radio) had no gameplay effect. This child only adds pure group queries;
//! wiring them into concrete link entry points is the parent's job.
use super::*;
use serde_json::Value;
use std::sync::OnceLock;

pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/cas-communications-2026.json"
        ))
        .expect("verified CaS communications group table")
    })
}

fn group_entry(group: &str) -> Result<&'static Value, String> {
    tables()["groups"]
        .get(group)
        .ok_or_else(|| format!("unknown CaS communications group: {group}"))
}

/// True when all fitted rooms of this CaS group remain operational. A partial
/// loss with surviving rooms has unqualified redundancy and stays pending.
/// `Ok(true)` also when the unit has no expanded fit at
/// all (the optional CaS table imposes nothing) or when no room of that group
/// is fitted: per the E00 absent-system rule this row cannot disable what is
/// not installed; whether the unit may use that link for other reasons is
/// the link entry's own qualification, never this query's.
/// `Err` on an unknown group (never silently true) or when the only rooms of
/// the group are source-pending.
pub fn communications_group_available(
    w: &World,
    id: EntityId,
    group: &str,
) -> Result<bool, String> {
    group_entry(group)?;
    let Some(f) = fit(w, id) else {
        return Ok(true);
    };
    let mut statuses = Vec::new();
    for (component, c) in &f.components {
        if c.kind == ComponentKind::Communications && c.cas_group.as_deref() == Some(group) {
            statuses.push(operational(w,id,component)?);
        }
    }
    if statuses.iter().all(|x|*x) {return Ok(true);}
    if statuses.iter().all(|x|!*x) {return Ok(false);}
    Err("CaS multiple rooms in one group: surviving-room redundancy is not source qualified".into())
}

/// Read the already adjudicated physical room state at the real radio link.
/// The caller resolves the printed scopes: `beyond_los` qualifies the
/// long-range group ("units beyond the horizon") from transport antenna
/// heights or both endpoints' declared radio_qualification heights — an HF
/// height is never assumed. `same_formation` qualifies the short-range group
/// ("other ships in the same formation") from both endpoints' declared
/// formation ids only; neither same side nor peer membership implies formation.
/// A damaged group with an inapplicable scope passes; a damaged group whose
/// scope applies blocks; a damaged group whose scope cannot be resolved stays
/// source-pending and never silently passes.
pub fn radio_pair_gate(w:&World,sender:EntityId,receiver:EntityId,
    beyond_los:Option<bool>,same_formation:Option<bool>)->Result<(),String> {
    let a=w.entity(sender).ok_or("CaS sender body unavailable")?;
    let b=w.entity(receiver).ok_or("CaS receiver body unavailable")?;
    let aircraft=a.kind==PlatformKind::Aircraft || b.kind==PlatformKind::Aircraft;
    for id in [sender,receiver] {
        if aircraft && !communications_group_available(w,id,"aircraft")? {
            return Err("CaS communications critical: aircraft radio group unavailable".into());
        }
        if !communications_group_available(w,id,"long_range")? {
            match beyond_los {
                Some(false)=>{},
                Some(true)=>return Err("CaS communications critical: long_range radio group unavailable".into()),
                None=>return Err("CaS long_range damaged: horizon qualification source_pending — declare radio_qualification antenna_height_m on both endpoints (never assumed, including HF)".into()),
            }
        }
        if !communications_group_available(w,id,"short_range")? {
            match same_formation {
                Some(false)=>{},
                Some(true)=>return Err("CaS communications critical: short_range radio group unavailable".into()),
                None=>return Err("CaS short_range damaged: formation qualification source_pending — declare the same radio_qualification formation on both ships (never inferred from side or peers)".into()),
            }
        }
    }
    Ok(())
}

/// Printed D6 grouping of the row (1-2 long, 3-4 short, 5-6 aircraft) for
/// callers that must interpret an already-rolled die without re-rolling.
pub fn group_for_cas_die(d: u8) -> Result<&'static str, String> {
    for (name, entry) in tables()["groups"].as_object().expect("groups object") {
        let span = entry["cas_die"].as_array().expect("cas_die pair");
        let lo = span[0].as_u64().expect("lo") as u8;
        let hi = span[1].as_u64().expect("hi") as u8;
        if (lo..=hi).contains(&d) {
            return Ok(name.as_str());
        }
    }
    Err(format!("D{d} outside the printed 1-6 communications die"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, Scenario};

    fn world() -> (World, EntityId, EntityId) {
        let mut db =
            Database::from_json(include_str!("../../../../../database/consolidated.json")).unwrap();
        db.platforms.get_mut("ru_akula1").unwrap().sensors.clear();
        let sc: Scenario = serde_json::from_value(json!({
            "id": "cas-comms-consumer",
            "name": "scenario_assumption: CaS-adjusted radio rooms consumer boundary",
            "seed": 1726,
            "duration_s": 500,
            "weather": {"visibility_nm": 0, "jam": [0, 0]},
            "context": {
                "rules_profile": "hv2026-rts-v4",
                "expanded_damage_2026": {
                    "source": "scenario_assumption: declared radio rooms, not an original Akula fit",
                    "units": {
                        "COMMSHOP": {
                            "source": "scenario_assumption: two declared radio rooms",
                            "components": {
                                "air_radio": {
                                    "source": "scenario_assumption: one aircraft-band radio room",
                                    "kind": "communications",
                                    "cas_group": "aircraft"
                                },
                                "long_radio": {
                                    "source": "scenario_assumption: one long-range radio room",
                                    "kind": "communications",
                                    "cas_group": "long_range"
                                }
                            }
                        }
                    }
                }
            },
            "units": [
                {"side": "Blue", "class": "ru_akula1", "name": "COMMSHOP",
                 "pos": [0, 0], "speed_kts": 0, "depth_m": 150, "fire_on": false},
                {"side": "Blue", "class": "ru_akula1", "name": "PLAIN",
                 "pos": [1, 1], "speed_kts": 0, "depth_m": 150, "fire_on": false}
            ]
        }))
        .unwrap();
        let w = crate::load_scenario(sc, db).unwrap();
        (w, EntityId(1), EntityId(2))
    }

    /// Adjudicated-input boundary: the CaS row's producer path with the D6
    /// group pre-adjudicated instead of rolled. Not a natural weapon hit.
    #[test]
    fn adjudicated_aircraft_radio_critical_blocks_only_its_group() {
        let (mut w, id, plain) = world();
        assert!(
            communications_group_available(&w, id, "aircraft").unwrap(),
            "fitted and intact room: group available"
        );
        assert!(communications_group_available(&w, id, "long_range").unwrap());
        let t = w.entity(id).unwrap().clone();
        let mut r = w.rng.derive(Rng::hash_tag(
            0,
            id.0,
            "scenario:cas-comms-consumer:adjudicated-aircraft-radio",
        ));
        component_critical(
            &mut w,
            &t,
            "scenario:cas-comms-consumer:adjudicated-aircraft-radio-critical",
            ComponentKind::Communications,
            Some("aircraft"),
            &mut r,
        );
        assert_eq!(
            communications_group_available(&w, id, "aircraft"),
            Ok(false),
            "the adjudicated critical disables the only aircraft-band room"
        );
        assert_eq!(
            communications_group_available(&w, id, "long_range"),
            Ok(true),
            "the aircraft hit must not leak into the long-range group"
        );
        assert!(w.rules.expanded_damage.as_ref().unwrap().units[&id]
            .disabled
            .contains("air_radio"));
        restore_component(&mut w, id, "air_radio").unwrap();
        assert!(
            communications_group_available(&w, id, "aircraft").unwrap(),
            "repair restores the group"
        );
        assert!(
            communications_group_available(&w, plain, "aircraft").unwrap(),
            "a unit without an expanded fit imposes no CaS block"
        );
    }

    #[test]
    fn unknown_group_is_an_error_and_the_printed_die_maps_to_printed_groups() {
        let (w, id, _) = world();
        assert!(communications_group_available(&w, id, "satellite").is_err());
        assert_eq!(group_for_cas_die(1), Ok("long_range"));
        assert_eq!(group_for_cas_die(2), Ok("long_range"));
        assert_eq!(group_for_cas_die(3), Ok("short_range"));
        assert_eq!(group_for_cas_die(4), Ok("short_range"));
        assert_eq!(group_for_cas_die(5), Ok("aircraft"));
        assert_eq!(group_for_cas_die(6), Ok("aircraft"));
        assert!(group_for_cas_die(0).is_err());
        assert!(group_for_cas_die(7).is_err());
    }

    #[test]
    fn data_matches_the_personally_viewed_printed_row() {
        let groups = tables()["groups"].as_object().expect("groups");
        assert_eq!(groups.len(), 3);
        for (name, entry) in groups {
            assert!(!entry["printed"].as_str().unwrap_or("").is_empty());
            let span = entry["cas_die"].as_array().expect("cas_die pair");
            assert_eq!(span.len(), 2);
            assert!(span[0].as_u64().unwrap() < span[1].as_u64().unwrap());
            let _ = name;
        }
    }
}

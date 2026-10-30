//! §3.1.2. A declared braking maneuver, distinct from sustained astern.
use super::*;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub active: bool,
    pub sequence: u64,
    pub current_trial: Option<usize>,
    pub trials: Vec<Trial>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trial {
    pub identity: String,
    pub tick: i64,
    pub d100: u8,
    pub engineering_applied: bool,
    pub pending: Option<String>,
}
pub fn active(w: &World, id: EntityId) -> bool {
    configured(w, id)
        && w.rules
            .navigation
            .as_ref()
            .and_then(|s| s.units.get(&id))
            .and_then(|s| s.crash_back.as_ref())
            .is_some_and(|s| s.active)
}
pub fn order(w: &mut World, id: EntityId, enabled: bool) -> Result<(), String> {
    let e = w.entity(id).ok_or("3.1.2 missing navigation unit")?;
    let u = &w
        .rules
        .navigation
        .as_ref()
        .ok_or("3.1.2 source navigation required")?
        .units[&id];
    if !matches!(
        e.kind,
        crate::types::PlatformKind::Ship | crate::types::PlatformKind::Sub
    ) {
        return Err("3.1.2 requires a ship or submarine".into());
    }
    if enabled {
        if u.crash_back.as_ref().is_some_and(|s| s.active) {
            return Ok(());
        }
        if e.speed_kts <= 0. {
            return Err("3.1.2 stationary unit has no crash-back deceleration".into());
        }
        if (u.quiet && e.kind == crate::types::PlatformKind::Sub)
            || u.coasting
            || u.heave_to
            || astern::engaged(w, id)
        {
            return Err("3.1.2 quiet/coasting/heave-to/astern combination is not adjudicated; explicitly end it first".into());
        }
        if u.crash_back
            .as_ref()
            .and_then(|s| s.trials.last())
            .is_some_and(|t| t.pending.is_some())
        {
            return Err(
                "3.1.2 original maneuver casualty is pending; a new order cannot discard it".into(),
            );
        }
        if super::super::damage_runtime::bridge_holds(e, w.tick)
            || e.ext.get("damage.rudder_jammed").is_some_and(|v| *v > 0.)
        {
            return Err(
                "3.1.2 damaged bridge/rudder maneuver combination needs adjudication".into(),
            );
        }
    }
    let state = w
        .rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap()
        .crash_back
        .get_or_insert_with(Default::default);
    state.active = enabled;
    if enabled {
        state.current_trial = None;
    }
    if enabled {
        let e = w.entity_mut(id).unwrap();
        e.ordered_speed_kts = 0.;
        e.waypoints.clear();
        e.station = None;
    } else {
        w.entity_mut(id)
            .unwrap()
            .ext
            .remove("navigation.crash_back_cavitation");
    }
    trace(
        w,
        id,
        "3.1.2.crash_back_order",
        json!({"enabled":enabled,"risk_policy":"one native D100 at first actual deceleration of this declared maneuver; repeated enabled is idempotent","source":"English2026 PDF23/3-1 section3.1.2"}),
    );
    Ok(())
}
/// Runs before the movement speed update. The original risk is saved before a
/// missing damage binding can suspend the maneuver; repeated ticks never redraw.
pub fn prepare(w: &mut World, id: EntityId) -> Result<(), String> {
    if !active(w, id) {
        return Ok(());
    }
    let e = w.entity(id).ok_or("3.1.2 unit missing")?.clone();
    if e.speed_kts <= 0. {
        w.rules
            .navigation
            .as_mut()
            .unwrap()
            .units
            .get_mut(&id)
            .unwrap()
            .crash_back
            .as_mut()
            .unwrap()
            .active = false;
        w.entity_mut(id)
            .unwrap()
            .ext
            .remove("navigation.crash_back_cavitation");
        return Ok(());
    }
    let u = &w.rules.navigation.as_ref().unwrap().units[&id];
    if (u.quiet && e.kind == crate::types::PlatformKind::Sub)
        || u.coasting
        || u.heave_to
        || astern::engaged(w, id)
    {
        return Err("3.1.2 active maneuver entered an unadjudicated navigation combination".into());
    }
    let state = u.crash_back.as_ref().unwrap();
    // A sequence belongs to one active maneuver. Cancellation does not erase
    // previous original dice; the next accepted maneuver gets the next identity.
    let needs_trial = state.current_trial.is_none();
    let pending = state
        .current_trial
        .and_then(|i| state.trials.get(i))
        .and_then(|t| t.pending.clone());
    if let Some(reason) = pending {
        return Err(reason);
    }
    if needs_trial {
        let identity = format!("3.1.2.crash_back:{}", state.sequence);
        let mut r = w.rng.derive(Rng::hash_tag(0, id.0, &identity));
        let d100 = 1 + (r.next_f64() * 100.) as u8;
        let now = w.tick;
        let state = w
            .rules
            .navigation
            .as_mut()
            .unwrap()
            .units
            .get_mut(&id)
            .unwrap()
            .crash_back
            .as_mut()
            .unwrap();
        state.sequence += 1;
        state.current_trial = Some(state.trials.len());
        state.trials.push(Trial {
            identity: identity.clone(),
            tick: now,
            d100,
            engineering_applied: false,
            pending: None,
        });
        let chance = tables()["crash_back"]["engineering_percent"]
            .as_u64()
            .unwrap() as u8;
        trace(
            w,
            id,
            "3.1.2.crash_back_trial",
            json!({"identity":identity,"d100":d100,"chance_percent":chance,"ignore_fire":true,"source":"English2026 PDF23/3-1 section3.1.2"}),
        );
        if d100 <= chance {
            let result = super::super::damage_runtime::crash_back_engineering(w, id, &identity);
            let trial = w
                .rules
                .navigation
                .as_mut()
                .unwrap()
                .units
                .get_mut(&id)
                .unwrap()
                .crash_back
                .as_mut()
                .unwrap()
                .trials
                .last_mut()
                .unwrap();
            match result {
                Ok(()) => trial.engineering_applied = true,
                Err(reason) => {
                    trial.pending = Some(reason.clone());
                    return Err(reason);
                }
            }
        }
    }
    w.entity_mut(id)
        .unwrap()
        .ext
        .insert("navigation.crash_back_cavitation".into(), 1.);
    Ok(())
}

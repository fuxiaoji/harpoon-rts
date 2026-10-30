//! Explicit source configuration is committed at the actual launch producer,
//! with the original new entity's loadout and ammunition. No generic aircraft
//! becomes unmanned merely because it is controlled by an AI.
use super::*;

fn domain_kind(domain: VehicleDomain) -> crate::types::PlatformKind {
    match domain {
        VehicleDomain::Air => crate::types::PlatformKind::Aircraft,
        VehicleDomain::Surface => crate::types::PlatformKind::Ship,
        VehicleDomain::Underwater => crate::types::PlatformKind::Sub,
    }
}
pub(super) fn validate_catalog(w: &World, s: &State) -> Result<(), String> {
    for (class, fit) in &s.config.launch_classes {
        validate_fit_numbers(fit)?;
        super::annex::validate(class, fit)?;
        let platform =
            w.db.platforms
                .get(class)
                .ok_or("2.5 exact launched class absent")?;
        if platform.kind != domain_kind(fit.domain) {
            return Err("2.5 launched physical kind and authored unmanned domain conflict".into());
        }
        if fit.initial_mode == Mode::Hitl && fit.controller.is_none() {
            return Err("2.5 launched HITL needs an actual author-assigned controller".into());
        }
        if let Some(controller) = &fit.controller {
            let id = named(w, controller)?;
            let cf = controller_for(w, s, id)?;
            if fit
                .element
                .as_ref()
                .is_none_or(|element| !cf.elements.contains(element))
            {
                return Err("2.5 launched source control element absent".into());
            }
        }
        if let LossBehavior::ReturnToHome { home } = &fit.loss_behavior {
            named(w, home)?;
        }
    }
    Ok(())
}

/// Invoke at queue admission and again immediately before actual entity/stock
/// mutation. A pending air job is not itself a new physical vehicle. Existing
/// reservations and live occupants are rechecked at actual launch completion.
pub fn validate_launch(w: &World, side: Side, class: &str, count: u8) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let Some(s) = w.rules.unmanned.as_ref() else {
        return Ok(());
    };
    check(w, s)?;
    let Some(fit) = s.config.launch_classes.get(class) else {
        return Ok(());
    };
    validate_fit_numbers(fit)?;
    super::annex::validate(class, fit)?;
    if count == 0 {
        return Err("2.5 positive launched vehicle count required".into());
    }
    if w.db
        .platforms
        .get(class)
        .is_none_or(|p| p.kind != domain_kind(fit.domain))
    {
        return Err("2.5 exact source launch class/kind missing".into());
    }
    if let Some(controller_name) = &fit.controller {
        let controller = named(w, controller_name)?;
        own(w, side, controller)?;
        let cf = controller_for(w, s, controller)?;
        let element = fit
            .element
            .as_deref()
            .ok_or("2.5 launched control element absent")?;
        if !cf.operational || disabled(s, controller) || !cf.elements.iter().any(|e| e == element) {
            return Err("2.5 launched own control element unavailable".into());
        }
        let occupied = occupancy(s, controller, element, EntityId(u32::MAX));
        let reserved: Vec<_> = s
            .vehicles
            .values()
            .filter_map(|v| v.transition.as_ref())
            .filter(|t| t.controller == Some(controller) && t.element.as_deref() == Some(element))
            .collect();
        let limit = if fit.initial_mode == Mode::Hitl {
            n("hitl_capacity")
        } else {
            n("autonomous_capacity")
        };
        if occupied.iter().any(|(_, mode)| *mode == Mode::Hitl)
            || !reserved.is_empty()
            || occupied.len() as i64 + i64::from(count) > limit
        {
            return Err("2.5 launched source control capacity is occupied or reserved".into());
        }
    } else if fit.initial_mode == Mode::Hitl {
        return Err("2.5 launched HITL controller missing".into());
    }
    if let LossBehavior::ReturnToHome { home } = &fit.loss_behavior {
        own(w, side, named(w, home)?)?;
    }
    Ok(())
}

/// Invoke once after inserting the genuine new physical entity and before any
/// sensor/weapon phase. Returns false for ordinary manned classes. The caller
/// must use validate_launch before any irreversible stock/entity mutation.
pub fn register_launched(w: &mut World, id: EntityId) -> Result<bool, String> {
    if !active(w) {
        return Ok(false);
    }
    let Some(entity) = w.entity(id).filter(|e| e.alive).cloned() else {
        return Err("2.5 actual newly launched entity missing".into());
    };
    let Some(s) = w.rules.unmanned.as_ref() else {
        return Ok(false);
    };
    check(w, s)?;
    let Some(fit) = s.config.launch_classes.get(&entity.class).cloned() else {
        return Ok(false);
    };
    if s.vehicles.contains_key(&id) {
        if s.launched_fits.contains_key(&id)
            && s.vehicles[&id].payload_fingerprint == payload(w, id)?
        {
            return Ok(true);
        }
        return Err("2.5 launched id already belongs to another hardware record".into());
    }
    validate_launch(w, entity.side, &entity.class, 1)?;
    let controller = fit
        .controller
        .as_ref()
        .map(|name| named(w, name))
        .transpose()?;
    let home_position = match &fit.loss_behavior {
        LossBehavior::ReturnToHome { home } => Some(own(w, entity.side, named(w, home)?)?.pos),
        LossBehavior::MoveAwayFromJammer => None,
    };
    let vehicle = VehicleState {
        name: entity.name.clone(),
        side: entity.side,
        class: entity.class.clone(),
        mode: fit.initial_mode,
        controller,
        element: fit.element.clone(),
        link: fit.initial_link,
        payload_fingerprint: payload(w, id)?,
        transition: None,
        mission: None,
        recall: None,
        mast_since: None,
        loss: None,
        last_wander_epoch: None,
        fuel_exhausted: false,
        home_position,
    };
    // All fallible qualification precedes state and fuel mutation.
    let s = w.rules.unmanned.as_mut().unwrap();
    s.launched_fits.insert(id, fit.clone());
    s.vehicles.insert(id, vehicle);
    if let Some(controller) = controller {
        s.known_roster.entry(controller).or_default().insert(id);
    }
    if let Some(seconds) = fit.endurance_s {
        w.entity_mut(id).unwrap().endurance_s = Some(seconds);
    }
    finish_telemetry(w);
    emit(
        w,
        id,
        "2.5.launched_hardware_registered",
        json!({"class":entity.class,"source":fit.source,"payload_selected_before_launch":true}),
        false,
    );
    Ok(true)
}

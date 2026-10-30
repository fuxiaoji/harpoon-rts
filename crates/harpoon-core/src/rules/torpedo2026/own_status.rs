//! §10.1 own inventory and §7.5.4 declarations, never projectile truth.
use super::*;
use crate::Side;

/// Public release settings are declarations, not an author's FCS assessment.
/// Keep the stored order intact for the referee and for old-save compatibility.
fn declared_order(order: Option<&Order>) -> Value {
    order.map_or(Value::Null, |o| {
        json!({"speed_index":o.speed_index,"depth_m":o.depth_m,
            "enable_run_nm":o.enable_run_nm,"bearing_deg":o.bearing_deg,
            "spread_deg":o.spread_deg,"cut_wire":o.cut_wire})
    })
}
fn launches(
    w: &World,
    side: Side,
    actor: Option<EntityId>,
    contact: Option<crate::ContactId>,
) -> Vec<Value> {
    let Some(state) = w.rules.torpedoes.as_ref() else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for (id, l) in &state.launches {
        if l.side != side
            || actor.is_some_and(|a| a != l.shooter)
            || contact.is_some_and(|c| Some(c) != l.contact)
        {
            continue;
        }
        result.push(json!({"id":id,"shooter":l.shooter,"weapon":l.weapon,"contact":l.contact,
            "launch_tick":l.launch_tick,"declared_order":declared_order(l.order.as_ref()),"guidance":l.guidance,"source":l.source,
            "physical_alive":null,"reported_status":null,
            "status":"unknown","status_reason":"No qualified status report is projected; a release declaration is not an active-weapon report."}));
    }
    // A previously saved v4 flight may predate the declaration ledger. Its
    // presence proves a release, but its alive flag and current homing state do
    // not prove shooter knowledge. Do not recover the weapon ID from a body
    // which might disappear and thereby change this fallback observation.
    for (id, f) in &state.flights {
        if state.launches.contains_key(id)
            || actor.is_some_and(|a| a != f.shooter)
            || contact.is_some()
            || w.entity(f.shooter).is_none_or(|e| e.side != side)
        {
            continue;
        }
        // No saved declaration means no proof of the original contact ID.
        // A newly detected track of the same referee body is not that proof.
        let contact: Option<crate::ContactId> = None;
        result.push(json!({"id":id,"shooter":f.shooter,"weapon":null,"contact":contact,
            "launch_tick":f.launch_tick,"declared_order":declared_order(Some(&f.order)),"guidance":f.weapon.guidance,"source":f.weapon.source,
            "physical_alive":null,"reported_status":null,"status":"unknown",
            "status_reason":"Historical source release without a declaration ledger or qualified status report."}));
    }
    result
}
pub fn unreported_releases(w: &World, side: Side, contact: crate::ContactId) -> Option<Value> {
    if !active(w) {
        return None;
    }
    let released = launches(w, side, None, Some(contact));
    if released.is_empty() {
        None
    } else {
        Some(json!({"status":"unknown","physical_in_flight":null,
        "released":released,"interpretation":"Known launch declarations; physical survival and in-flight count are unreported."}))
    }
}
/// Capture the shooter's known release at the common physical launch entry.
/// Rich source fire fills in declared orders afterwards. Observation never
/// recovers this identity from a body which may secretly disappear.
pub fn register_release(w: &mut World, projectile: EntityId) {
    if !active(w) {
        return;
    }
    let Some(body) = w
        .entity(projectile)
        .filter(|e| e.kind == PlatformKind::Torpedo)
    else {
        return;
    };
    let Some(p) = body.projectile.clone() else {
        return;
    };
    let contact = p
        .target
        .and_then(|target| w.find_contact(p.launched_by_side, target));
    let Some(state) = w.rules.torpedoes.as_mut() else {
        return;
    };
    let fit = state.config.weapons.get(&p.weapon);
    state.launches.insert(
        projectile,
        LaunchRecord {
            shooter: p.launched_by,
            side: p.launched_by_side,
            target: p.target,
            contact,
            weapon: p.weapon,
            launch_tick: w.tick,
            order: None,
            guidance: fit.map(|f| f.guidance.clone()),
            source: fit
                .map(|f| f.source.clone())
                .unwrap_or_else(|| "Source torpedo metadata not bound at this release".into()),
        },
    );
}
pub fn own_view(w: &World, side: Side, actor: EntityId) -> Option<Value> {
    if !active(w) {
        return None;
    }
    let e = w
        .entity(actor)
        .filter(|e| e.side == side && !e.kind.is_projectile())?;
    let state = w.rules.torpedoes.as_ref()?;
    let released = launches(w, side, Some(actor), None);
    let plans: Vec<_> = state
        .plans
        .values()
        .filter(|plan| plan.actor == actor && plan.side == side)
        .collect();
    // Source settings are available only for this actor's carried torpedo
    // mounts. Aggregate database speed/range is no substitute for an annex row.
    let mut weapon_profiles = BTreeMap::new();
    for mount in &e.weapons {
        if mount.mounts == 0
            || !w
                .db
                .weapons
                .get(&mount.weapon)
                .is_some_and(|d| matches!(d.kind, WeaponKind::Torpedo | WeaponKind::Lwt))
        {
            continue;
        }
        if let Some(p) = state.config.weapons.get(&mount.weapon) {
            weapon_profiles.insert(
                mount.weapon.clone(),
                json!({"source":p.source,"guidance":p.guidance,"generation":p.generation,
                    "settings":p.settings,"max_depth_m":p.max_depth_m,
                    "surface":p.surface,"submarine":p.submarine}),
            );
        }
    }
    let Some(fit) = state.tubes.get(&actor) else {
        return (e.kind==PlatformKind::Sub||!released.is_empty()||!plans.is_empty()||!weapon_profiles.is_empty()).then(||json!({"verified":false,
            "reason":"10.1 source physical tube fit not declared; aggregate mount ammo is not a tube log.",
            "tubes":null,"reserves":null,"reloads":null,"releases":released,
            "plans":plans,"weapon_profiles":weapon_profiles}));
    };
    let tubes:Vec<_>=fit.tubes.iter().enumerate().map(|(i,t)|{
        let physical_id=fit.tube_ids.iter().find(|(_,index)|**index==i).map(|(id,_)|id);
        json!({"index":i,"physical_id":physical_id,"loaded":t.loaded,"bearing_deg":t.bearing_deg,
            "servicing":state.reloads.iter().any(|j|j.actor==actor&&j.tube==i)})
    }).collect();
    let reloads: Vec<_> = state
        .reloads
        .iter()
        .filter(|j| j.actor == actor)
        .map(|j| {
            json!({"tube":j.tube,"weapon":j.weapon,"due":j.due,"remaining_s":(j.due-w.tick).max(0),
            "changing":j.changing,"purpose":j.purpose})
        })
        .collect();
    Some(
        json!({"verified":true,"source":fit.source,"inventory_scope":"source physical tubes and reserves; mount ammo is a separate aggregate",
        "tubes":tubes,"reserves":fit.reserves,"reloads":reloads,"releases":released,
        "plans":plans,"weapon_profiles":weapon_profiles,
        "feedback_boundary":"Dual-wire may return status under7.5.4; exact report precision and timing are not implemented. Raw projectile position, phase, target truth and survival are not reports."}),
    )
}
fn fit_index(w: &World, actor: EntityId, tube_id: &str) -> Result<usize, String> {
    if !active(w) {
        return Err("10/13 physical tube API requiresv4".into());
    }
    w.entity(actor)
        .filter(|e| e.alive && e.kind == PlatformKind::Sub)
        .ok_or("10/13 own operational submarine missing")?;
    let state = w
        .rules
        .torpedoes
        .as_ref()
        .ok_or("10/13 source tube runtime missing")?;
    let fit = state
        .tubes
        .get(&actor)
        .ok_or("10/13 source physical tube fit missing")?;
    if fit.source.trim().is_empty() {
        return Err("10/13 source physical tube fit missing".into());
    }
    let index = *fit
        .tube_ids
        .get(tube_id)
        .ok_or("10/13 explicit physical tube ID mapping missing")?;
    if index >= fit.tubes.len() {
        return Err("10/13 physical tube ID outside source inventory".into());
    }
    Ok(index)
}
/// Resolve a source ID only when the physical tube is loaded and free of
/// servicing and an uncut guidance wire. The caller still checks its load.
pub fn tube_ready(w: &World, actor: EntityId, tube_id: &str) -> Result<usize, String> {
    let index = fit_index(w, actor, tube_id)?;
    let state = w.rules.torpedoes.as_ref().unwrap();
    if state
        .reloads
        .iter()
        .any(|j| j.actor == actor && j.tube == index)
    {
        return Err("10/13 physical tube is being serviced".into());
    }
    if state
        .flights
        .values()
        .any(|f| f.shooter == actor && f.tube == Some(index) && f.wire)
    {
        return Err("7.5.4 cut the physical tube guidance wire first".into());
    }
    if state.tubes[&actor].tubes[index].loaded.is_none() {
        return Err("10/13 physical tube is empty".into());
    }
    Ok(index)
}
/// Includes a reserved reload already removed from reserve, so initialization
/// and saved-game checks use one physical inventory rather than two magazines.
pub fn mine_inventory(w: &World, actor: EntityId, mine_weapon: &str) -> Result<u32, String> {
    if !active(w) {
        return Err("10/13 physical tube API requiresv4".into());
    }
    let state = w
        .rules
        .torpedoes
        .as_ref()
        .ok_or("10/13 source tube runtime missing")?;
    let fit = state
        .tubes
        .get(&actor)
        .ok_or("10/13 source physical tube fit missing")?;
    if fit.source.trim().is_empty() {
        return Err("10/13 source physical tube fit missing".into());
    }
    let loaded = fit
        .tubes
        .iter()
        .filter(|t| t.loaded.as_deref() == Some(mine_weapon))
        .count() as u32;
    let reserved = state
        .reloads
        .iter()
        .filter(|j| j.actor == actor && j.weapon == mine_weapon)
        .count() as u32;
    Ok(loaded + reserved + u32::from(fit.reserves.get(mine_weapon).copied().unwrap_or(0)))
}
/// All IDs and loads are validated before any mutation. A discharged mine
/// leaves the same tube empty; an existing physical reserve is reserved for
/// reload, never created. The source mine rule supplies its360-second clock.
pub fn occupy_mine_tubes(
    w: &mut World,
    actor: EntityId,
    tube_ids: &[String],
    mine_weapon: &str,
    reload_seconds: i64,
) -> Result<(), String> {
    if tube_ids.is_empty() || mine_weapon.trim().is_empty() || reload_seconds <= 0 {
        return Err("13 source mine tube occupation invalid".into());
    }
    let mut indexes = BTreeSet::new();
    for id in tube_ids {
        let index = tube_ready(w, actor, id)?;
        if !indexes.insert(index) {
            return Err("13 duplicate physical mine tube".into());
        }
        if w.rules.torpedoes.as_ref().unwrap().tubes[&actor].tubes[index]
            .loaded
            .as_deref()
            != Some(mine_weapon)
        {
            return Err("13 physical tube is not loaded with the declared mine".into());
        }
    }
    let state = w.rules.torpedoes.as_ref().unwrap();
    let fit = &state.tubes[&actor];
    let busy = state.reloads.iter().filter(|j| j.actor == actor).count();
    let stock = fit.reserves.get(mine_weapon).copied().unwrap_or(0) as usize;
    let new_jobs = indexes.len().min(stock);
    if state.reloads.iter().any(|j| j.actor == actor && j.changing)
        || busy + new_jobs > fit.reload_parallel as usize
    {
        return Err("10/13 source tube-room concurrent servicing limit".into());
    }
    let due = w
        .tick
        .checked_add(reload_seconds)
        .ok_or("13 source mine reload clock overflow")?;
    let state = w.rules.torpedoes.as_mut().unwrap();
    let fit = state.tubes.get_mut(&actor).unwrap();
    for index in indexes {
        fit.tubes[index].loaded = None;
        if let Some(stock) = fit.reserves.get_mut(mine_weapon).filter(|n| **n > 0) {
            *stock -= 1;
            state.reloads.push(Reload {
                actor,
                tube: index,
                weapon: mine_weapon.into(),
                due,
                changing: false,
                purpose: Some("mine_tube_reload".into()),
            });
        }
    }
    trace(
        w,
        actor,
        "13.source_mine_tubes",
        json!({"physical_tube_ids":tube_ids,"weapon":mine_weapon,"reload_seconds":reload_seconds,"due":due}),
    );
    Ok(())
}

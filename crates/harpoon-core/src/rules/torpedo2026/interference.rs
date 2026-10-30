//! PDF79 / printed7-4, 7.5.3. Source mutual guidance, not an invented explosion.
use super::*;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Runtime {
    pub prepared_tick: i64,
    pub pairs: BTreeMap<String, Pair>,
    pub pending: BTreeMap<EntityId, Pending>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pair {
    pub identity: String,
    pub members: [EntityId; 2],
    pub weapon: String,
    pub locked_at: i64,
    pub capture_settings: [usize; 2],
    pub fastest_settings: [usize; 2],
    pub last_advanced_tick: Option<i64>,
    pub approach_seconds: f64,
    pub meeting_at: Option<i64>,
    pub meeting_fraction: Option<f64>,
    pub pending: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pending {
    pub at: i64,
    pub members: Vec<EntityId>,
    pub reason: String,
}
fn pair_for<'a>(runtime: &'a Runtime, id: EntityId) -> Option<&'a Pair> {
    runtime
        .pairs
        .values()
        .find(|pair| pair.members.contains(&id))
}
fn searching(f: &Flight) -> bool {
    f.acquisition.is_none()
        && f.opportunity.is_none()
        && matches!(f.phase, Phase::Run | Phase::Search { .. })
}
fn active_seeker(w: &World, id: EntityId, f: &Flight) -> bool {
    w.entity(id).is_some_and(|e| e.alive)
        && f.launch_tick < w.tick
        && f.weapon.guidance.acoustic()
        && f.weapon.guidance != Guidance::Passive
        && f.run_nm >= f.order.enable_run_nm
        && f.fuel < 1.
}
fn fastest(f: &Flight) -> usize {
    f.weapon
        .settings
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.speed_kts.total_cmp(&b.speed_kts))
        .map(|(index, _)| index)
        .unwrap_or(f.speed_index)
}
fn pending(w: &mut World, runtime: &mut Runtime, members: &[EntityId], reason: &str) {
    let changed = members.iter().any(|id| !runtime.pending.contains_key(id));
    for id in members {
        runtime.pending.entry(*id).or_insert_with(|| Pending {
            at: w.tick,
            members: members.to_vec(),
            reason: reason.into(),
        });
    }
    if changed {
        referee_trace(
            w,
            members[0],
            "7.5.3.interference_pending",
            json!({"members":members,"reason":reason,"source":"2026 PDF79 printed7-4 section7.5.3","no_terminal_damage_inferred":true}),
        );
    }
}

/// A synchronized tick snapshot includes all launches, including flights later
/// temporarily removed by the ordinary per-projectile executor. Canonical
/// weapon IDs, not equality of capability payloads, define "same type".
pub(super) fn prepare(w: &mut World) {
    let Some(mut state) = w.rules.torpedoes.take() else {
        return;
    };
    let mut runtime = state.interference.take().unwrap_or_default();
    if runtime.prepared_tick == w.tick {
        state.interference = Some(runtime);
        w.rules.torpedoes = Some(state);
        return;
    }
    runtime.prepared_tick = w.tick;
    let ids: Vec<_> = state
        .flights
        .iter()
        .filter_map(|(id, f)| active_seeker(w, *id, f).then_some(*id))
        .collect();
    let mut graph: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();
    // Preserve existing capture edges when looking for a later competitor.
    // Their members cannot disappear from the acoustic candidate snapshot.
    for pair in runtime.pairs.values().filter(|p| p.pending.is_none()) {
        if pair.members.iter().all(|id| ids.contains(id)) {
            graph
                .entry(pair.members[0])
                .or_default()
                .insert(pair.members[1]);
            graph
                .entry(pair.members[1])
                .or_default()
                .insert(pair.members[0]);
        }
    }
    let limit = tables()["interference"]["distance_yards"]
        .as_f64()
        .expect("verified source distance")
        * tables()["constants"]["yards_to_nm"].as_f64().unwrap();
    for (index, a) in ids.iter().enumerate() {
        for b in ids.iter().skip(index + 1) {
            let fa = &state.flights[a];
            let fb = &state.flights[b];
            if !searching(fa) && !searching(fb) {
                continue;
            }
            let ea = w.entity(*a).unwrap();
            let eb = w.entity(*b).unwrap();
            if ea.class != eb.class || ea.pos.range_to(eb.pos) > limit + 1e-12 {
                continue;
            }
            graph.entry(*a).or_default().insert(*b);
            graph.entry(*b).or_default().insert(*a);
        }
    }
    let mut visited = BTreeSet::new();
    for start in graph.keys().copied() {
        if visited.contains(&start) {
            continue;
        }
        let mut members = BTreeSet::new();
        let mut stack = vec![start];
        while let Some(id) = stack.pop() {
            if members.insert(id) {
                stack.extend(graph.get(&id).into_iter().flatten().copied());
            }
        }
        visited.extend(members.iter().copied());
        let members: Vec<_> = members.into_iter().collect();
        if members.iter().any(|id| {
            w.rules.pending_adjudications.contains_key(id) || runtime.pending.contains_key(id)
        }) {
            pending(w,&mut runtime,&members,"existing unresolved source adjudication retained; mutual guidance cannot resume a frozen weapon");
            continue;
        }
        if members.len() != 2 {
            pending(
                w,
                &mut runtime,
                &members,
                "multiple same-type active competitors; source pair allocation is not specified",
            );
            continue;
        }
        if pair_for(&runtime, members[0]).is_some_and(|pair| pair.members.contains(&members[1])) {
            continue; // one original capture identity, not a new pair each tick
        }
        let a = w.entity(members[0]).unwrap();
        let b = w.entity(members[1]).unwrap();
        let environment_known = state.config.environment.as_ref().is_some_and(|env| {
            !env.source.trim().is_empty() && env.layer_m.is_none() && !env.reverberant
        });
        if (a.depth_m - b.depth_m).abs() > 1e-9 || !environment_known {
            pending(w, &mut runtime, &members, "noncoplanar or unresolved acoustic environment; no vertical teleport or signal multiplier inferred");
            continue;
        }
        let identity = format!("7.5.3.pair:{}:{}:{}", members[0].0, members[1].0, w.tick);
        let pair = Pair {
            identity: identity.clone(),
            members: [members[0], members[1]],
            weapon: a.class.clone(),
            locked_at: w.tick,
            capture_settings: [
                state.flights[&members[0]].speed_index,
                state.flights[&members[1]].speed_index,
            ],
            fastest_settings: [
                fastest(&state.flights[&members[0]]),
                fastest(&state.flights[&members[1]]),
            ],
            last_advanced_tick: None,
            approach_seconds: 0.,
            meeting_at: None,
            meeting_fraction: None,
            pending: None,
        };
        let distance = a.pos.range_to(b.pos);
        for (index, id) in members.iter().enumerate() {
            referee_trace(
                w,
                *id,
                "7.5.3.same_type_interference",
                json!({"identity":identity,"torpedo":id,
                "other":members[1-index],"range_nm":distance,"weapon":pair.weapon,
                "source":"2026 PDF79 printed7-4 section7.5.3","policy":"synchronized private mutual guidance; original launch and target retained"}),
            );
        }
        runtime.pairs.insert(identity, pair);
    }
    // Empty preparation is not a new fact in historical/source worlds.
    state.interference =
        (!runtime.pairs.is_empty() || !runtime.pending.is_empty()).then_some(runtime);
    w.rules.torpedoes = Some(state);
}

/// Both members are advanced once by one common epoch, so entity iteration
/// order cannot let the second weapon chase the first weapon's future pose.
pub(super) fn advance(w: &mut World, state: &mut State, id: EntityId) -> Result<bool, String> {
    let Some(mut runtime) = state.interference.take() else {
        return Ok(false);
    };
    if runtime.pending.contains_key(&id) {
        state.interference = Some(runtime);
        return Ok(true);
    }
    let Some(key) = pair_for(&runtime, id).map(|pair| pair.identity.clone()) else {
        state.interference = Some(runtime);
        return Ok(false);
    };
    let mut pair = runtime.pairs[&key].clone();
    if pair.pending.is_some() || pair.last_advanced_tick == Some(w.tick) {
        state.interference = Some(runtime);
        return Ok(true);
    }
    let alive = pair.members.iter().all(|member| {
        w.entity(*member).is_some_and(|e| e.alive) && state.flights.contains_key(member)
    });
    if !alive {
        let reason = "captured partner lost; source retarget/recovery not specified";
        pair.pending = Some(reason.into());
        pending(w, &mut runtime, &pair.members, reason);
        runtime.pairs.insert(key, pair);
        state.interference = Some(runtime);
        return Ok(true);
    }
    let positions = pair.members.map(|member| w.entity(member).unwrap().pos);
    let selected = if w.tick == pair.locked_at {
        pair.capture_settings
    } else {
        pair.fastest_settings
    };
    let settings: [Setting; 2] =
        [0, 1].map(|i| state.flights[&pair.members[i]].weapon.settings[selected[i]].clone());
    let distance = positions[0].range_to(positions[1]);
    let closure = (settings[0].speed_kts + settings[1].speed_kts) / 3600.;
    let mut duration = if closure > 0. {
        (distance / closure).min(1.)
    } else {
        1.
    };
    for i in 0..2 {
        let flight = &state.flights[&pair.members[i]];
        let budget_seconds =
            (1. - flight.fuel).max(0.) * settings[i].range_nm * 3600. / settings[i].speed_kts;
        duration = duration.min(budget_seconds);
    }
    let before = pair.approach_seconds;
    for i in 0..2 {
        let member = pair.members[i];
        let course = positions[i].bearing_to(positions[1 - i]);
        let travel = settings[i].speed_kts / 3600. * duration;
        let f = state.flights.get_mut(&member).unwrap();
        f.speed_index = selected[i];
        f.run_nm += travel;
        f.fuel = (f.fuel + travel / settings[i].range_nm).min(1.);
        let e = w.entity_mut(member).unwrap();
        e.pos = positions[i].advanced(course, travel);
        e.course_deg = course;
        e.speed_kts = settings[i].speed_kts;
        if let Some(projectile) = e.projectile.as_mut() {
            projectile.live_s += duration;
        }
    }
    pair.last_advanced_tick = Some(w.tick);
    pair.approach_seconds += duration;
    referee_trace(
        w,
        id,
        "7.5.3.interference_approach",
        json!({"identity":pair.identity,"members":pair.members,
        "distance_before_nm":distance,"duration_s":duration,"settings":selected,
        "approach_seconds_before":before,"source_motion":true}),
    );
    if closure > 0. && distance <= closure * duration + 1e-12 {
        let reason="mutual seekers reached the same physical point; source gives no torpedo-on-torpedo terminal/explosion rule";
        pair.meeting_at = Some(w.tick);
        pair.meeting_fraction = Some(duration);
        pair.pending = Some(reason.into());
        pending(w, &mut runtime, &pair.members, reason);
    } else {
        for member in pair.members {
            if state.flights[&member].fuel >= 1. {
                referee_trace(
                    w,
                    member,
                    "10.2.source_runout",
                    json!({"torpedo":member,"spent_fraction":state.flights[&member].fuel,"interference":pair.identity}),
                );
                retire(w, member);
            }
        }
    }
    runtime.pairs.insert(key, pair);
    state.interference = Some(runtime);
    Ok(true)
}

/// Adjudication snapshots are saved after the runtime is returned to World.
pub(super) fn pending_target(state: &State, id: EntityId) -> Option<EntityId> {
    let runtime = state.interference.as_ref()?;
    let pending = runtime.pending.get(&id)?;
    // A multi-competitor group has no sourced unique acoustic target. Its
    // complete members stay in runtime/evidence; the scalar keeps the original
    // assignment as context rather than inventing an ID-selected pairing.
    if pending.members.len() == 2 {
        if let Some(pair) = pair_for(runtime, id) {
            if pair
                .members
                .iter()
                .all(|member| pending.members.contains(member))
            {
                return Some(if pair.members[0] == id {
                    pair.members[1]
                } else {
                    pair.members[0]
                });
            }
        }
    }
    state.flights.get(&id).map(|flight| flight.target)
}

pub(super) fn pending_records(state: &State) -> Vec<(EntityId, EntityId, String)> {
    state
        .interference
        .as_ref()
        .map_or_else(Vec::new, |runtime| {
            runtime
                .pending
                .iter()
                .map(|(id, p)| {
                    let target = pending_target(state, *id).unwrap_or(*id);
                    (*id, target, p.reason.clone())
                })
                .collect()
        })
}

//! §10.2.2–10.2.3 proportional movement and one die per surviving spread.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Motion {
    pub from: Vec2,
    pub to: Vec2,
    pub tick: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Salvo {
    pub id: EntityId,
    pub shooter: EntityId,
    pub origin: Vec2,
    pub members: Vec<EntityId>,
    pub launch_tick: i64,
    pub launch_positions: BTreeMap<EntityId, Vec2>,
    pub last_tick: i64,
    pub split: bool,
    pub visited: BTreeMap<EntityId, BTreeSet<EntityId>>,
    pub attacks: Vec<SalvoAttack>,
    pub pending: Option<(EntityId, String)>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SalvoAttack {
    pub tick: i64,
    pub target: EntityId,
    pub members: Vec<EntityId>,
    pub d100: u8,
    pub hits: u8,
    pub hit_members: Vec<EntityId>,
    pub effective_size: String,
    pub run_nm: f64,
    pub aspect_deg: f64,
}
#[derive(Clone, Copy)]
struct Plan {
    from: Vec2,
    to: Vec2,
    travel: f64,
    expired: bool,
}
struct Encounter {
    target: EntityId,
    members: Vec<EntityId>,
    fraction: f64,
    target_pos: Vec2,
    torpedo_pos: Vec2,
}

fn at(from: Vec2, to: Vec2, t: f64) -> Vec2 {
    from.add(to.sub(from).scaled(t))
}
/// Relative swept segments, implementing the printed proportional-movement test.
fn closest(p0: Vec2, p1: Vec2, t0: Vec2, t1: Vec2) -> (f64, f64) {
    let r = p0.sub(t0);
    let d = p1.sub(p0).sub(t1.sub(t0));
    let denominator = d.x * d.x + d.y * d.y;
    let fraction = if denominator > 1e-24 {
        (-(r.x * d.x + r.y * d.y) / denominator).clamp(0., 1.)
    } else {
        0.
    };
    (r.add(d.scaled(fraction)).norm(), fraction)
}
fn average(plans: &BTreeMap<EntityId, Plan>, members: &[EntityId]) -> (Vec2, Vec2) {
    let mut from = Vec2::zero();
    let mut to = Vec2::zero();
    for id in members {
        from = from.add(plans[id].from);
        to = to.add(plans[id].to);
    }
    (
        from.scaled(1. / members.len() as f64),
        to.scaled(1. / members.len() as f64),
    )
}
fn target_motion(
    w: &World,
    state: &State,
    salvo: &Salvo,
    target: &crate::world::Entity,
) -> (Vec2, Vec2) {
    if salvo.launch_tick == w.tick {
        return (
            salvo
                .launch_positions
                .get(&target.id)
                .copied()
                .unwrap_or(target.pos),
            target.pos,
        );
    }
    state
        .motion
        .get(&target.id)
        .filter(|m| m.tick == w.tick)
        .map(|m| (m.from, m.to))
        .unwrap_or((target.pos, target.pos))
}
fn next_encounter(
    w: &World,
    state: &State,
    salvo: &Salvo,
    plans: &BTreeMap<EntityId, Plan>,
    after: f64,
) -> Option<Encounter> {
    let live: Vec<_> = salvo
        .members
        .iter()
        .copied()
        .filter(|id| plans.contains_key(id) && w.entity(*id).is_some_and(|e| e.alive))
        .collect();
    if live.is_empty() {
        return None;
    }
    let weapon = &state.flights[&live[0]].weapon;
    let packs: Vec<Vec<EntityId>> = if salvo.split {
        live.iter().map(|id| vec![*id]).collect()
    } else {
        vec![live]
    };
    let radius = tables()["constants"]["straight_attack_radius_nm"]
        .as_f64()
        .unwrap();
    let mut candidates = Vec::new();
    for target in w
        .entities
        .values()
        .filter(|e| e.alive && e.id != salvo.shooter && !e.kind.is_projectile())
    {
        let domain = target.combat_kind() == PlatformKind::Ship && weapon.surface
            || target.kind == PlatformKind::Sub
                && target.depth_m <= tables()["depth_limits_m"][0].as_f64().unwrap()
                && weapon.submarine;
        if !domain || target.depth_m > weapon.max_depth_m {
            continue;
        }
        let (t0, t1) = target_motion(w, state, salvo, target);
        for members in &packs {
            if members.iter().any(|id| {
                salvo
                    .visited
                    .get(id)
                    .is_some_and(|set| set.contains(&target.id))
            }) {
                continue;
            }
            let (p0, p1) = average(plans, members);
            let (distance, fraction) = closest(p0, p1, t0, t1);
            if distance <= radius && fraction + 1e-12 >= after {
                candidates.push(Encounter {
                    target: target.id,
                    members: members.clone(),
                    fraction,
                    target_pos: at(t0, t1, fraction),
                    torpedo_pos: at(p0, p1, fraction),
                });
            }
        }
    }
    candidates.sort_by(|a, b| {
        a.fraction
            .total_cmp(&b.fraction)
            .then(a.target.cmp(&b.target))
            .then(a.members[0].cmp(&b.members[0]))
    });
    candidates.into_iter().next()
}
fn mark_visited(salvo: &mut Salvo, members: &[EntityId], target: EntityId) {
    for id in members {
        salvo.visited.entry(*id).or_default().insert(target);
    }
}

pub(super) fn advance(
    w: &mut World,
    state: &mut State,
    id: EntityId,
    def: &WeaponDef,
) -> Result<(), String> {
    let key = state
        .flights
        .get(&id)
        .and_then(|f| f.salvo)
        .ok_or("10.2.3 stable spread identity missing")?;
    let mut salvo = state
        .salvos
        .remove(&key)
        .ok_or("10.2.3 source spread state missing")?;
    if salvo.last_tick == w.tick {
        state.salvos.insert(key, salvo);
        return Ok(());
    }
    if let Some((_, reason)) = &salvo.pending {
        let reason = reason.clone();
        state.salvos.insert(key, salvo);
        return Err(reason);
    }
    let result = advance_inner(w, state, &mut salvo, def);
    if let Err((target, reason)) = &result {
        salvo.pending = Some((*target, reason.clone()));
    }
    state.salvos.insert(key, salvo);
    result.map_err(|(_, reason)| reason)
}
fn advance_inner(
    w: &mut World,
    state: &mut State,
    salvo: &mut Salvo,
    def: &WeaponDef,
) -> Result<(), (EntityId, String)> {
    salvo.last_tick = w.tick;
    let mut plans = BTreeMap::new();
    for id in &salvo.members {
        let Some(e) = w.entity(*id).filter(|e| e.alive) else {
            continue;
        };
        let f = state.flights.get(id).ok_or((
            salvo.shooter,
            "10.2.3 spread member lost its source flight".into(),
        ))?;
        let setting = &f.weapon.settings[f.speed_index];
        let travel = (setting.speed_kts / 3600.).min(((1. - f.fuel) * setting.range_nm).max(0.));
        plans.insert(
            *id,
            Plan {
                from: e.pos,
                to: e.pos.advanced(e.course_deg, travel),
                travel,
                expired: f.fuel + travel / setting.range_nm >= 1. - 1e-12,
            },
        );
    }
    // Move the surviving weapons once, even though each has a normal engine
    // projectile entry. The next member's advance sees the same last_tick.
    for (id, plan) in &plans {
        let f = state.flights.get_mut(id).unwrap();
        let setting = &f.weapon.settings[f.speed_index];
        f.run_nm += plan.travel;
        f.fuel += plan.travel / setting.range_nm;
        let e = w.entity_mut(*id).unwrap();
        e.pos = plan.to;
        e.speed_kts = setting.speed_kts;
        if let Some(p) = e.projectile.as_mut() {
            p.live_s += 1.;
        }
    }
    let mut after = 0.;
    while let Some(event) = next_encounter(w, state, salvo, &plans, after) {
        let target = w
            .entity(event.target)
            .cloned()
            .ok_or((event.target, "10.2.3 swept target missing".into()))?;
        // This swept gyro encounter consumes physical size/foil only.
        let spec =
            fitted_target_spec(w, state, event.target).map_err(|reason| (event.target, reason))?;
        mark_visited(salvo, &event.members, event.target);
        after = event.fraction;
        if spec.foil_borne || spec.size >= 5 {
            referee_trace(
                w,
                salvo.shooter,
                "10.2.3.immune_target",
                json!({"salvo":salvo.id,"target":event.target,"foil_borne":spec.foil_borne,"size":spec.size}),
            );
            continue;
        }
        let mean_course = w
            .entity(event.members[0])
            .map(|e| e.course_deg)
            .unwrap_or(0.);
        let incoming = if event.torpedo_pos.range_to(event.target_pos) > 1e-9 {
            event.target_pos.bearing_to(event.torpedo_pos)
        } else {
            mean_course + 180.
        };
        let aspect = angle_diff(target.course_deg, incoming);
        let effective =
            effective_size(spec.size, aspect).map_err(|reason| (event.target, reason))?;
        let run = salvo.origin.range_to(event.target_pos);
        let purpose = if salvo.split {
            format!("10.2.3.single:{}", event.target.0)
        } else {
            format!("10.2.3.salvo:{}:{}", event.target.0, salvo.attacks.len())
        };
        let die_id = if salvo.split {
            event.members[0]
        } else {
            salvo.id
        };
        let d100 = rng_die(w, die_id, &purpose, 100);
        let hits = straight_hits(
            effective,
            event.members.len(),
            run,
            target.speed_kts == 0.,
            d100,
        )
        .map_err(|reason| (event.target, reason))?;
        // The source table gives a hit count, not individual member identities.
        // Resolve those identities by closest proportional approach, then ID;
        // this deterministic RTS choice never changes the table's hit count.
        let (t0, t1) = target_motion(w, state, salvo, &target);
        let mut ranked = event.members.clone();
        ranked.sort_by(|a, b| {
            let a_plan = plans[a];
            let b_plan = plans[b];
            closest(a_plan.from, a_plan.to, t0, t1)
                .0
                .total_cmp(&closest(b_plan.from, b_plan.to, t0, t1).0)
                .then(a.cmp(b))
        });
        let hit_members: Vec<_> = ranked.into_iter().take(hits as usize).collect();
        for member in &event.members {
            state.flights.get_mut(member).unwrap().last_d100 = Some(d100);
        }
        let attack = SalvoAttack {
            tick: w.tick,
            target: event.target,
            members: event.members.clone(),
            d100,
            hits,
            hit_members: hit_members.clone(),
            effective_size: effective.into(),
            run_nm: run,
            aspect_deg: aspect,
        };
        referee_trace(
            w,
            salvo.shooter,
            "10.2.3.source_salvo",
            json!({"salvo":salvo.id,"target":attack.target,"members":attack.members,"d100":d100,"hits":hits,"hit_members":hit_members,"effective_size":effective,"run_nm":run,"aspect_deg":aspect,"stationary":target.speed_kts==0.,"split":salvo.split,"source":"HV2026 PDF110–113/10.2.2–10.2.3"}),
        );
        salvo.attacks.push(attack);
        let shooter_side = w
            .entity(salvo.shooter)
            .map(|e| e.side)
            .unwrap_or(crate::Side::Blue);
        for member in hit_members {
            if let Some(e) = w.entity_mut(member) {
                e.pos = target.pos;
            }
            crate::damage::apply_projectile_hit(w, member, event.target, def, true, shooter_side);
            retire(w, member);
        }
        if hits > 0 {
            salvo.split = true;
        }
    }
    for (id, plan) in plans {
        if plan.expired && w.entity(id).is_some_and(|e| e.alive) {
            referee_trace(
                w,
                salvo.shooter,
                "10.2.3.source_runout",
                json!({"salvo":salvo.id,"torpedo":id}),
            );
            retire(w, id);
        }
    }
    Ok(())
}

//! §7.5.5/§10.2.6 explicit wake geometry and source following.
//! Geometry width/lifetime/history are authored inputs, never book defaults.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchOffset {
    pub source: String,
    pub degrees: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Configuration {
    pub source: String,
    pub half_width_nm: f64,
    pub lifetime_s: i64,
    pub history_policy: String,
    pub initial_segments: Vec<InitialSegment>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InitialSegment {
    pub target_unit: String,
    pub from: Vec2,
    pub to: Vec2,
    pub created_tick: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Segment {
    pub id: u64,
    pub target: EntityId,
    pub from: Vec2,
    pub to: Vec2,
    pub created_tick: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Crossing {
    pub segment: u64,
    pub target: EntityId,
    pub at: Vec2,
    pub tick: i64,
    pub pass: u32,
}
/// Referee-only, stable per weapon/pass. Anti-decoy and target attack are
/// separate source rolls; a seduced weapon never receives an attack roll.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Terminal {
    pub target: EntityId,
    pub pass: u32,
    pub towed: Option<EntityId>,
    pub towed_generation: Option<u8>,
    pub anti_probability: Option<f64>,
    pub anti_d100: Option<u8>,
    pub ignored: Option<bool>,
    pub evading: bool,
    pub probability: Option<f64>,
    pub d100: Option<u8>,
    pub hit: Option<bool>,
    pub applied: bool,
}
/// A physically encountered branch is retained for the referee until the
/// crossing-vessel selection and changed wake path are source-bound. This is
/// neither a guessed decoy roll nor a global broadcast of every wake crossing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Junction {
    pub followed_segment: u64,
    pub crossing_segment: u64,
    pub crossing_ship: EntityId,
    pub at: Vec2,
    pub crossing_tick: i64,
    pub encountered_tick: i64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub next_segment: u64,
    pub segments: Vec<Segment>,
    pub following: BTreeMap<EntityId, u64>,
    pub crossings: BTreeMap<EntityId, Vec<Crossing>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub terminals: BTreeMap<String, Terminal>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pending_junctions: BTreeMap<EntityId, Vec<Junction>>,
}
fn finite(p: Vec2) -> bool {
    p.x.is_finite() && p.y.is_finite()
}
pub fn initialize(w: &World, c: &Configuration) -> Result<State, String> {
    if c.source.trim().is_empty()
        || !c.half_width_nm.is_finite()
        || c.half_width_nm <= 0.
        || c.lifetime_s <= 0
        || c.history_policy != "explicit_segments_only"
    {
        return Err(
            "7.5.5 explicit source/scenario wake width, lifetime and prehistory policy required"
                .into(),
        );
    }
    let mut s = State::default();
    for initial in &c.initial_segments {
        if initial.target_unit.trim().is_empty()
            || !finite(initial.from)
            || !finite(initial.to)
            || initial.created_tick > w.tick
            || initial.from.range_to(initial.to) <= 1e-12
        {
            return Err("7.5.5 authored wake history invalid".into());
        }
        let targets: Vec<_> = w
            .entities
            .values()
            .filter(|e| e.name == initial.target_unit && e.kind == PlatformKind::Ship)
            .collect();
        if targets.len() != 1 {
            return Err("7.5.5 authored wake history needs one exact ship binding".into());
        }
        s.next_segment += 1;
        s.segments.push(Segment {
            id: s.next_segment,
            target: targets[0].id,
            from: initial.from,
            to: initial.to,
            created_tick: initial.created_tick,
        });
    }
    Ok(s)
}
pub fn step(tick: i64, ships: &BTreeSet<EntityId>, state: &mut super::State) {
    let Some(c) = state.config.wake.as_ref() else {
        return;
    };
    let Some(s) = state.wakes.as_mut() else {
        return;
    };
    s.segments
        .retain(|seg| tick - seg.created_tick <= c.lifetime_s);
    for (id, m) in &state.motion {
        if m.tick != tick || m.from.range_to(m.to) <= 1e-12 || !ships.contains(id) {
            continue;
        }
        s.next_segment += 1;
        s.segments.push(Segment {
            id: s.next_segment,
            target: *id,
            from: m.from,
            to: m.to,
            created_tick: tick,
        });
    }
}
fn point_segment(p: Vec2, a: Vec2, b: Vec2) -> (f64, f64) {
    let d = b.sub(a);
    let denominator = d.x * d.x + d.y * d.y;
    let t = if denominator > 1e-24 {
        ((p.x - a.x) * d.x + (p.y - a.y) * d.y) / denominator
    } else {
        0.
    }
    .clamp(0., 1.);
    (p.range_to(a.add(d.scaled(t))), t)
}
/// Minimum distance of two swept segments and the fraction along the weapon's
/// segment. Endpoint tests plus an exact line intersection cover capsules.
fn swept(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> (f64, f64) {
    let r = b.sub(a);
    let s = d.sub(c);
    let det = r.x * s.y - r.y * s.x;
    if det.abs() > 1e-24 {
        let q = c.sub(a);
        let t = (q.x * s.y - q.y * s.x) / det;
        let u = (q.x * r.y - q.y * r.x) / det;
        if (0. ..=1.).contains(&t) && (0. ..=1.).contains(&u) {
            return (0., t);
        }
    }
    let (da, _) = point_segment(a, c, d);
    let (db, _) = point_segment(b, c, d);
    let (dc, tc) = point_segment(c, a, b);
    let (dd, td) = point_segment(d, a, b);
    [(da, 0.), (db, 1.), (dc, tc), (dd, td)]
        .into_iter()
        .min_by(|x, y| x.0.total_cmp(&y.0).then(x.1.total_cmp(&y.1)))
        .unwrap()
}
fn first_crossing(
    w: &World,
    s: &super::State,
    target: EntityId,
    from: Vec2,
    to: Vec2,
) -> Option<(Segment, Vec2, f64)> {
    let config = s.config.wake.as_ref()?;
    s.wakes
        .as_ref()?
        .segments
        .iter()
        .filter(|seg| seg.target == target && w.tick - seg.created_tick <= config.lifetime_s)
        .filter_map(|seg| {
            let (distance, t) = swept(from, to, seg.from, seg.to);
            (distance <= config.half_width_nm + 1e-12)
                .then(|| (seg.clone(), from.add(to.sub(from).scaled(t)), t))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2).then(a.0.id.cmp(&b.0.id)))
}
fn guard_junction(
    w: &mut World,
    s: &mut State,
    id: EntityId,
    target: EntityId,
    followed: &Segment,
    from: Vec2,
    to: Vec2,
    half_width_nm: f64,
) -> Result<(), String> {
    let junctions: Vec<_> = s
        .segments
        .iter()
        .filter(|other| other.target != target && other.created_tick >= followed.created_tick)
        .filter_map(|other| {
            // First establish a real other-ship trajectory crossing the original
            // wake after it existed, then require this weapon to reach that branch.
            // A nearby ship, a crossing elsewhere, or a junction behind this weapon
            // does not satisfy these two local geometric predicates.
            let (crossing_distance, along) =
                swept(followed.from, followed.to, other.from, other.to);
            if crossing_distance > half_width_nm + 1e-12 {
                return None;
            }
            let at = followed
                .from
                .add(followed.to.sub(followed.from).scaled(along));
            let (encounter_distance, _) = point_segment(at, from, to);
            (encounter_distance <= half_width_nm + 1e-12).then_some(Junction {
                followed_segment: followed.id,
                crossing_segment: other.id,
                crossing_ship: other.target,
                at,
                crossing_tick: other.created_tick,
                encountered_tick: w.tick,
            })
        })
        .collect();
    if junctions.is_empty() {
        return Ok(());
    }
    s.pending_junctions.insert(id, junctions.clone());
    referee_trace(
        w,
        id,
        "7.5.5.wake_junction_pending",
        json!({
            "torpedo":id,"target":target,"from":from,"to":to,"junctions":junctions,
            "geometry":"explicit authored wake capsule; source crossing-vessel ACM procedure and changed path not yet enabled"
        }),
    );
    Err("7.5.5 crossing-vessel wake junction physically encountered: source decoy selection and changed wake path pending; no original-target terminal shortcut".into())
}
fn follow(
    w: &mut World,
    state: &mut super::State,
    id: EntityId,
    target: EntityId,
    mut pos: Vec2,
    mut travel: f64,
) -> Result<(Vec2, f64, bool), String> {
    let half_width = state
        .config
        .wake
        .as_ref()
        .ok_or("7.5.5 wake geometry missing")?
        .half_width_nm;
    let s = state.wakes.as_mut().ok_or("7.5.5 wake runtime not bound")?;
    let mut current = *s
        .following
        .get(&id)
        .ok_or("7.5.5 acquired wake segment not recorded")?;
    let mut heading = w.entity(id).ok_or("wake torpedo missing")?.course_deg;
    for _ in 0..=s.segments.len() {
        let segment = s
            .segments
            .iter()
            .find(|seg| seg.id == current)
            .cloned()
            .ok_or("7.5.5 followed wake expired; source reacquisition unresolved")?;
        let distance = pos.range_to(segment.to);
        if distance > travel + 1e-12 {
            heading = pos.bearing_to(segment.to);
            let next = pos.advanced(heading, travel);
            guard_junction(w, s, id, target, &segment, pos, next, half_width)?;
            pos = next;
            s.following.insert(id, current);
            return Ok((pos, heading, false));
        }
        if distance > 1e-12 {
            heading = pos.bearing_to(segment.to);
            guard_junction(w, s, id, target, &segment, pos, segment.to, half_width)?;
            pos = segment.to;
            travel = (travel - distance).max(0.);
        }
        let next = s
            .segments
            .iter()
            .filter(|seg| {
                seg.target == target
                    && seg.id != current
                    && (seg.created_tick, seg.id) > (segment.created_tick, segment.id)
                    && seg.from.range_to(segment.to) <= 1e-9
            })
            .min_by_key(|seg| (seg.created_tick, seg.id))
            .map(|seg| seg.id);
        if let Some(next) = next {
            current = next;
            continue;
        }
        s.following.insert(id, current);
        // Homing follows the wake chain; no shortcut from its endpoint to a
        // hidden ship position is invented when authored history has a gap.
        if w.entity(target)
            .is_some_and(|t| pos.range_to(t.pos) <= 1e-9)
        {
            return Ok((pos, heading, true));
        }
        return Err("7.5.5 wake chain does not reach source ship; missing geometry cannot become direct homing".into());
    }
    Err("7.5.5 wake chain cycle or unresolved ordering".into())
}
fn record_crossing(
    w: &mut World,
    state: &mut super::State,
    id: EntityId,
    f: &Flight,
    segment: &Segment,
    at: Vec2,
) -> Result<(), String> {
    let crossing = Crossing {
        segment: segment.id,
        target: f.target,
        at,
        tick: w.tick,
        pass: f.pass,
    };
    let wake = state.wakes.as_mut().ok_or("7.5.5 wake state missing")?;
    wake.following.insert(id, segment.id);
    wake.crossings.entry(id).or_default().push(crossing.clone());
    referee_trace(
        w,
        f.shooter,
        "10.2.6.wake_crossing",
        json!({"torpedo":id,"crossing":crossing}),
    );
    Ok(())
}
fn roll_placement(
    w: &mut World,
    state: &mut super::State,
    id: EntityId,
    f: &mut Flight,
    gate: &str,
    range_nm: f64,
) -> Result<(), String> {
    let launch = state.launches.get(&id)
        .ok_or("6.3.8 Wake placement group metadata absent: complete flight pending, no guessed per-projectile die")?;
    let turn_s = tables()["constants"]["wake_placement_group_tactical_seconds"]
        .as_i64()
        .unwrap();
    let launch_turn = launch.launch_tick.div_euclid(turn_s);
    let cohort: Vec<_> = state.launches.iter().filter(|(_, other)| {
        other.shooter == launch.shooter && other.weapon == launch.weapon
            && other.launch_tick.div_euclid(turn_s) == launch_turn
    }).map(|(member, other)| json!({"id":member,"launch_tick":other.launch_tick,"contact":other.contact})).collect();
    if cohort.len() > 1 {
        // Stable own launch declarations survive secret loss and cleanup. The
        // set must not shrink to one when a hidden projectile stops running.
        referee_trace(
            w,
            f.shooter,
            "6.3.8.wake_group_pending",
            json!({
                "torpedo":id,"shooter":launch.shooter,"weapon":launch.weapon,
                "launch_turn":launch_turn,"tactical_seconds":turn_s,"cohort":cohort,
                "gate":gate,"range_nm":range_nm,
                "status":"Wake same-unit/type/turn shared placement not reconciled; no guessed separate or common die"
            }),
        );
        return Err("6.3.8 multiple Wake source launches in the same unit/type/Tactical-Turn group: shared placement pending before any guessed die".into());
    }
    let env = state
        .config
        .environment
        .as_ref()
        .ok_or("7.5.6 source environment missing")?;
    let modifier = tables()["constants"]["wake_placement_modifier"]
        .as_i64()
        .unwrap() as i32
        + if env.reverberant && !f.weapon.shallow_water_capable {
            if f.weapon.generation <= 3 {
                -2
            } else {
                -1
            }
        } else {
            0
        };
    let placement = super::super::firecontrol2026::placement(
        w,
        &mut state.placements,
        &format!("10.wake_placement:{}:{}", id.0, f.pass),
        f.quality,
        modifier,
    )?;
    referee_trace(
        w,
        f.shooter,
        "10.2.6.wake_placement",
        json!({
            "torpedo":id,"pass":f.pass,"placement":placement,"modifier":modifier,
            "gate":gate,"range_nm":range_nm,"initial_half_distance_nm":f.initial_range / 2.,
            "source":"6.3.8/10.2.4 initial half-distance;10.2.7 reacquisition;10.2.6 Wake modifier"
        }),
    );
    f.placement = Some(placement.clone());
    if placement.placed {
        Ok(())
    } else {
        Err("6.3.8/10.2.6 failed source Wake placement: original deviation dice and complete flight retained; eligible alternate-wake acquisition unresolved".into())
    }
}
pub fn advance(
    w: &mut World,
    state: &mut super::State,
    id: EntityId,
    def: &WeaponDef,
) -> Result<(), String> {
    let mut f = state
        .flights
        .remove(&id)
        .ok_or("10.2.6 source Wake flight missing")?;
    let result = advance_flight(w, state, id, def, &mut f);
    state.flights.insert(id, f);
    result
}
fn advance_flight(
    w: &mut World,
    state: &mut super::State,
    id: EntityId,
    def: &WeaponDef,
    f: &mut Flight,
) -> Result<(), String> {
    let target = w
        .entity(f.target)
        .filter(|e| e.alive)
        .cloned()
        .ok_or("10.2.6 target lost; physical wake source unresolved")?;
    if target.combat_kind() != PlatformKind::Ship {
        return Err(
            "10.2.6 physical target is not a surface ship; source wake reassignment required"
                .into(),
        );
    }
    // Wake geometry consumes physical foil state, not an acoustic signature.
    let tp = fitted_target_spec(w, state, target.id)?;
    let e = w.entity(id).cloned().ok_or("10.2.6 torpedo missing")?;
    let setting = f.weapon.settings[f.speed_index].clone();
    // The 75% made-good movement represents a zigzag at the source attack
    // speed, not a 25% discount on engine endurance. Returning toward the
    // launch point can never replenish the already spent range fraction.
    let source_step = setting.speed_kts / 3600.;
    let remaining = (1. - f.fuel).max(0.) * setting.range_nm;
    let nominal = source_step.min(remaining);
    let progress = tables()["constants"]["wake_progress"].as_f64().unwrap();
    if f.threat.is_none()
        && super::super::detection2026::known_category(w, target.side, id)
            == Some(PlatformKind::Torpedo)
        && w.sides[target.side.index()]
            .contacts
            .values()
            .any(|c| c.about == id && !c.stale)
    {
        f.threat = Some(Threat {
            course: target.course_deg,
            speed: target.speed_kts,
            depth_band: depth_band(target.depth_m)?,
        });
    }
    let mut pos = e.pos;
    let mut course = e.course_deg;
    let mut reached = false;
    match f.phase.clone() {
        Phase::Run | Phase::Search { .. } => {
            let next = pos.advanced(course, nominal);
            let crossing = if e.projectile.as_ref().is_some_and(|p| p.enabled)
                || f.run_nm >= f.order.enable_run_nm
            {
                first_crossing(w, state, f.target, pos, next)
            } else {
                None
            };
            if f.pass == 0 && f.placement.is_none() {
                // §6.3.8/§10.2.4 place the initial weapon after half-distance,
                // independently of seeker activation. A wake met before that
                // completed gate has no verified combined procedure here.
                if let Some((segment, at, _)) = crossing.as_ref() {
                    record_crossing(w, state, id, f, segment, *at)?;
                    return Err("6.3.8/10.2.4 early Wake crossing before the half-distance placement gate: complete flight preserved; no early die or homing".into());
                }
                let after_run = f.origin.range_to(next);
                if after_run >= f.initial_range / 2. {
                    roll_placement(
                        w,
                        state,
                        id,
                        f,
                        "initial_half_distance_first_continuous_tick",
                        after_run,
                    )?;
                }
            }
            if let Some((seg, at, fraction)) = crossing {
                record_crossing(w, state, id, f, &seg, at)?;
                if f.placement.is_none() {
                    // A later attack repeats placement on actual reacquisition;
                    // it does not reapply half the original launch range.
                    let reattack_range = f.origin.range_to(at);
                    roll_placement(
                        w,
                        state,
                        id,
                        f,
                        "reattack_actual_wake_reacquisition",
                        reattack_range,
                    )?;
                }
                if !f.placement.as_ref().is_some_and(|p| p.placed) {
                    return Err("10.2.6 Wake capture has no successful source placement".into());
                }
                f.phase = Phase::Locked;
                let advanced = follow(
                    w,
                    state,
                    id,
                    f.target,
                    at,
                    nominal * (1. - fraction) * progress,
                )?;
                pos = advanced.0;
                course = advanced.1;
                reached = advanced.2;
            } else {
                pos = next;
            }
        }
        Phase::Locked => {
            let advanced = follow(w, state, id, f.target, pos, nominal * progress)?;
            pos = advanced.0;
            course = advanced.1;
            reached = advanced.2;
        }
        Phase::Past { remaining_nm } => {
            let distance = nominal.min(remaining_nm);
            pos = pos.advanced(course, distance);
            let remaining = (remaining_nm - distance).max(0.);
            if remaining <= 1e-12 {
                let turn = tables()["constants"]["wake_reattack_right_turn_deg"]
                    .as_f64()
                    .unwrap();
                course = (course + turn).rem_euclid(360.);
                f.phase = Phase::Search {
                    center: pos,
                    heading: course,
                    angle: 0.,
                };
                state
                    .wakes
                    .as_mut()
                    .ok_or("7.5.5 wake state missing")?
                    .following
                    .remove(&id);
                referee_trace(
                    w,
                    f.shooter,
                    "10.2.7.wake_reverse",
                    json!({"torpedo":id,"pass":f.pass,"at":pos,"right_turn_deg":turn,
                        "past_yards":tables()["constants"]["wake_reattack_past_yards"],"course_deg":course}),
                );
            } else {
                f.phase = Phase::Past {
                    remaining_nm: remaining,
                };
            }
        }
    }
    f.run_nm += nominal;
    f.fuel = (f.fuel + nominal / setting.range_nm).min(1.);
    if f.fuel >= 1. - 1e-12 {
        f.fuel = 1.;
    }
    if let Some(body) = w.entity_mut(id) {
        body.pos = pos;
        body.course_deg = course;
        body.depth_m = f.order.depth_m;
        body.speed_kts = setting.speed_kts;
        if let Some(p) = body.projectile.as_mut() {
            p.live_s += nominal / source_step;
            p.enabled |= f.run_nm >= f.order.enable_run_nm;
        }
    }
    if reached {
        if tp.foil_borne || target.depth_m > f.weapon.max_depth_m {
            return Err(
                "10.2.6 physical target immune to current Wake run; source continuation unresolved"
                    .into(),
            );
        }
        terminal(w, state, id, def, f, &target)?;
    }
    if f.fuel >= 1. && w.entity(id).is_some_and(|body| body.alive) {
        referee_trace(
            w,
            f.shooter,
            "10.2.6.wake_runout",
            json!({"torpedo":id,"origin_range_nm":f.origin.range_to(pos),
                "cumulative_run_nm":f.run_nm,"spent_fraction":f.fuel,"source_range_nm":setting.range_nm}),
        );
        retire(w, id);
    }
    Ok(())
}

fn terminal(
    w: &mut World,
    state: &mut super::State,
    id: EntityId,
    def: &WeaponDef,
    f: &mut Flight,
    target: &crate::world::Entity,
) -> Result<(), String> {
    let key = format!("{}:{}", id.0, f.pass);
    if state
        .wakes
        .as_ref()
        .and_then(|s| s.terminals.get(&key))
        .is_some_and(|r| r.applied)
    {
        return Ok(());
    }
    let defenses = super::super::ew2026::active_acms(w, target.id);
    let generations = tables()["constants"]["wake_towed_generations"]
        .as_array()
        .unwrap();
    let eligible: Vec<_> = defenses
        .iter()
        .filter(|d| {
            d.towed
                && generations
                    .iter()
                    .any(|g| g.as_u64() == Some(d.generation as u64))
        })
        .collect();
    // The source describes one towed device. It supplies neither a priority
    // nor a compounding rule for multiple effective objects.
    if eligible.len() > 1 {
        let candidates: Vec<_> = eligible
            .iter()
            .map(|d| json!({"id":d.id,"generation":d.generation}))
            .collect();
        referee_trace(
            w,
            f.shooter,
            "10.2.6.wake_towed_pending",
            json!({"torpedo":id,"pass":f.pass,"target":target.id,"candidates":candidates}),
        );
        return Err("10.2.6 multiple eligible towed objects: source selection/roll count unresolved; no guessed priority or dice".into());
    }
    let evading = target.max_speed(&w.db)
        >= tables()["constants"]["wake_evasion_minimum_kts"]
            .as_f64()
            .unwrap()
        && f.threat
            .as_ref()
            .is_some_and(|threat| evasion(threat, target, true));
    let mut record = Terminal {
        target: target.id,
        pass: f.pass,
        towed: None,
        towed_generation: None,
        anti_probability: None,
        anti_d100: None,
        ignored: None,
        evading,
        probability: None,
        d100: None,
        hit: None,
        applied: false,
    };
    if let Some(decoy) = eligible.first() {
        let probability =
            homing_probability(f.weapon.generation, Some(decoy.generation as usize), false)?;
        let d100 = rng_die(w, id, &format!("10.2.6.towed:{}", f.pass), 100);
        let ignored = source_percent_hit(probability, d100);
        record.towed = Some(decoy.id);
        record.towed_generation = Some(decoy.generation);
        record.anti_probability = Some(probability);
        record.anti_d100 = Some(d100);
        record.ignored = Some(ignored);
        referee_trace(
            w,
            f.shooter,
            "10.2.6.wake_towed",
            json!({"torpedo":id,"pass":f.pass,"target":target.id,"towed":decoy.id,
                "generation":decoy.generation,"probability":probability,"d100":d100,"ignored":ignored}),
        );
        if !ignored {
            super::super::ew2026::consume_towed(w, decoy.id);
            retire(w, id);
            record.applied = true;
            state
                .wakes
                .as_mut()
                .ok_or("7.5.5 wake state missing")?
                .terminals
                .insert(key, record);
            return Ok(());
        }
    }
    let probability = homing_probability(f.weapon.generation, None, evading)?
        + if evading {
            tables()["constants"]["wake_evasion_add"].as_f64().unwrap()
        } else {
            0.
        };
    // Source Ph cells and the additive Wake correction use whole percents.
    // Preserve the printed decimal (0.56+0.05=0.61) in the saved trace.
    let probability = (probability * 100.).round() / 100.;
    let d100 = rng_die(w, id, &format!("10.2.6.attack:{}", f.pass), 100);
    let hit = source_percent_hit(probability, d100);
    f.last_probability = Some(probability);
    f.last_d100 = Some(d100);
    record.probability = Some(probability);
    record.d100 = Some(d100);
    record.hit = Some(hit);
    record.applied = true;
    state
        .wakes
        .as_mut()
        .ok_or("7.5.5 wake state missing")?
        .terminals
        .insert(key, record);
    referee_trace(
        w,
        f.shooter,
        "10.2.6.wake_terminal",
        json!({"torpedo":id,"pass":f.pass,"target":target.id,"probability":probability,
            "d100":d100,"evading":evading,"hit":hit}),
    );
    if hit {
        let side = w
            .entity(id)
            .ok_or("10.2.6 source torpedo missing at physical hit")?
            .side;
        crate::damage::apply_projectile_hit(w, id, target.id, def, true, side);
        retire(w, id);
    } else {
        f.pass += 1;
        f.placement = None;
        if !defenses.is_empty() || evading {
            f.quality=match f.quality {
                Quality::Good=>Quality::Fair,
                Quality::Fair=>Quality::Poor,
                _=>return Err("10.2.7 Poor reattack floor conflicts with10-6 example NoAttack; original Wake terminal dice retained".into()),
            };
        }
        f.phase = Phase::Past {
            remaining_nm: yards_nm(
                tables()["constants"]["wake_reattack_past_yards"]
                    .as_f64()
                    .unwrap(),
            ),
        };
    }
    Ok(())
}

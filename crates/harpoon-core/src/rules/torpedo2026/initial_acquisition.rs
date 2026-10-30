//! A1: one complete first opportunity acquisition, coplanar throughout the
//! radial capture tick. Source6.3.8/7.5/10.2; one-second epochs and acceleration
//! on the next tick are explicit RTS conventions, not new source constants.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DepthMotion {
    pub from: Option<f64>,
    pub to: f64,
    pub tick: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FirstAttack {
    pub tick: i64,
    pub d100: u8,
    pub probability: f64,
    pub hit: bool,
    pub evading: bool,
    pub acm_generation: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcquisitionRecord {
    pub revision: String,
    pub original_target: EntityId,
    pub target: EntityId,
    pub owner: crate::Side,
    pub tick: i64,
    pub fraction: f64,
    pub mode: String,
    pub position: Vec2,
    pub target_position: Vec2,
    pub depth_m: f64,
    pub search_speed_index: usize,
    pub attack_speed_index: usize,
    pub pursuit_started_at: Option<i64>,
    pub threat: Option<Threat>,
    pub terminal: Option<FirstAttack>,
}

fn dot(a: Vec2, b: Vec2) -> f64 {
    a.x * b.x + a.y * b.y
}
fn finite(p: Vec2) -> bool {
    p.x.is_finite() && p.y.is_finite()
}
fn roots(a: f64, b: f64, c: f64, points: &mut Vec<f64>) {
    if a.abs() < 1e-24 {
        if b.abs() > 1e-24 {
            let root = -c / b;
            if (0. ..=1.).contains(&root) {
                points.push(root);
            }
        }
        return;
    }
    let discriminant = b * b - 4. * a * c;
    if discriminant < 0. {
        return;
    }
    let root = discriminant.sqrt();
    for value in [(-b - root) / (2. * a), (-b + root) / (2. * a)] {
        if (0. ..=1.).contains(&value) {
            points.push(value);
        }
    }
}

/// Relative linear motion with a source range and horizontal +/-60deg cone.
/// Polynomial boundaries retain even a short interval between tick endpoints.
fn first_eligible(
    from: Vec2,
    to: Vec2,
    heading: f64,
    target_from: Vec2,
    target_to: Vec2,
    range: f64,
) -> Option<f64> {
    let forward = Vec2::zero().advanced(heading, 1.);
    let relative = target_from.sub(from);
    let velocity = target_to.sub(target_from).sub(to.sub(from));
    let r2 = dot(relative, relative);
    let v2 = dot(velocity, velocity);
    let rv = dot(relative, velocity);
    let along = dot(relative, forward);
    let along_v = dot(velocity, forward);
    let mut boundaries = vec![0., 1.];
    roots(v2, 2. * rv, r2 - range * range, &mut boundaries);
    roots(
        4. * along_v * along_v - v2,
        8. * along * along_v - 2. * rv,
        4. * along * along - r2,
        &mut boundaries,
    );
    roots(0., along_v, along, &mut boundaries);
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    let eligible = |t: f64| {
        let r = relative.add(velocity.scaled(t));
        let length2 = dot(r, r);
        let ahead = dot(r, forward);
        length2 <= range * range + 1e-12 && ahead >= -1e-12 && 4. * ahead * ahead + 1e-12 >= length2
    };
    for (index, &start) in boundaries.iter().enumerate() {
        if eligible(start)
            || boundaries
                .get(index + 1)
                .is_some_and(|end| eligible((start + end) / 2.))
        {
            return Some(start);
        }
    }
    None
}

pub(super) fn select(
    w: &mut World,
    state: &State,
    id: EntityId,
    f: &Flight,
    h: &HelicalState,
    usable: f64,
    speed: f64,
) -> Result<Option<AcquisitionRecord>, String> {
    let body = w.entity(id).ok_or("7.5.6 A1 torpedo body missing")?.clone();
    let travel = speed * usable;
    let radius = yards_nm(
        tables()["constants"]["air_search_diameter_yards"]
            .as_f64()
            .unwrap(),
    ) / 2.;
    // Horizontal radial geometry is independent of depth qualification, also
    // during the zero-time entry check. Same-depth/full-tick acceptance belongs
    // to the separate capture scope.
    let radial = h.radial_run_nm + travel < radius && h.circle_turns == 0.;
    let body_to = body.pos.advanced(h.heading_deg, travel);
    let instantaneous_entry = usable == 0.
        && h.elapsed_s == 0.
        && f.airdrop_entry
            .as_ref()
            .is_some_and(|entry| entry.tick == w.tick && entry.resolved_at == Some(w.tick));
    let active = f.weapon.guidance != Guidance::Passive;
    let outer = acquisition_nm(f.weapon.generation, true, 0, false, false, true)?.max(
        acquisition_nm(f.weapon.generation, false, 0, false, false, true)?,
    );
    let mut qualified = Vec::new();
    let mut evidence = Vec::new();
    let mut unknown = false;
    for candidate in w.entities.values().filter(|e| e.alive && e.id != id) {
        if candidate.id == f.target && f.placement.as_ref().is_some_and(|p| !p.placed) {
            continue;
        }
        let domain = if !instantaneous_entry && candidate.kind == PlatformKind::Sub {
            // A submarine completing its native surface operation is a Ship
            // only at the endpoint. Its underwater portion still competes in
            // this epoch. Missing history cannot prove either domain absent.
            let depth = state.depth_motion.get(&candidate.id).filter(|depth| {
                depth.tick == w.tick
                    && depth.to == candidate.depth_m
                    && depth.to.is_finite()
                    && depth.to >= 0.
                    && depth
                        .from
                        .is_some_and(|from| from.is_finite() && from >= 0.)
            });
            let possible_sub = depth.is_none_or(|d| d.from.unwrap() > 0. || d.to > 0.);
            let possible_surface = depth.is_none_or(|d| d.from.unwrap() == 0. || d.to == 0.);
            (f.weapon.submarine && possible_sub) || (f.weapon.surface && possible_surface)
        } else {
            // A real instantaneous entry has no earlier physical interval.
            match candidate.combat_kind() {
                PlatformKind::Ship => f.weapon.surface,
                PlatformKind::Sub => f.weapon.submarine,
                _ => false,
            }
        };
        if !domain {
            continue;
        }
        // An end depth beyond the source limit cannot hide an earlier reachable
        // part of this epoch. Missing depth history is not proof of exclusion.
        let entirely_beyond_max = if instantaneous_entry {
            candidate.depth_m.is_finite() && candidate.depth_m > f.weapon.max_depth_m
        } else {
            state.depth_motion.get(&candidate.id).is_some_and(|depth| {
                depth.tick == w.tick
                    && depth.to == candidate.depth_m
                    && depth.to.is_finite()
                    && depth.to > f.weapon.max_depth_m
                    && depth
                        .from
                        .is_some_and(|from| from.is_finite() && from > f.weapon.max_depth_m)
            })
        };
        if entirely_beyond_max {
            continue;
        }
        let distance = if instantaneous_entry {
            if !finite(candidate.pos) {
                return Err(
                    "7.5.6 A1 instantaneous entry position unknown; complete decision pending"
                        .into(),
                );
            }
            body.pos.range_to(candidate.pos)
        } else {
            helical::candidate_distance(w, state, body.pos, candidate)?
        };
        if distance > outer + travel {
            continue;
        }
        let frame = json!({"candidate":candidate.id,"position":candidate.pos,"depth_m":candidate.depth_m,
            "physical_kind":candidate.kind,"current_combat_kind":candidate.combat_kind(),
            "instantaneous_entry":instantaneous_entry,
            "motion":state.motion.get(&candidate.id),"depth_motion":state.depth_motion.get(&candidate.id)});
        let (candidate_from, candidate_to) = if instantaneous_entry {
            // This zero-duration check occurs at actual water entry. A path
            // before entry is not a future search opportunity for this body.
            // Potential candidates at this instant are still checked below.
            (candidate.pos, candidate.pos)
        } else {
            let Some(motion) = state.motion.get(&candidate.id).filter(|m| {
                m.tick == w.tick
                    && finite(m.from)
                    && finite(m.to)
                    && m.to == candidate.pos
                    && (m.from != m.to || candidate.speed_kts == 0.)
            }) else {
                unknown = true;
                evidence.push(json!({"frame":frame,"reason":"missing actual candidate path"}));
                continue;
            };
            (motion.from, motion.to)
        };
        // A proven horizontal exclusion is safe even before unknown acoustic
        // metadata/vertical rules are considered. Do not globally hold on it.
        if radial
            && first_eligible(
                body.pos,
                body_to,
                h.heading_deg,
                candidate_from,
                candidate_to,
                outer,
            )
            .is_none()
        {
            continue;
        }
        let profile = match target_spec(w, state, candidate.id) {
            Ok(profile) if !profile.source.trim().is_empty() => profile,
            _ => {
                unknown = true;
                evidence.push(json!({"frame":frame,"reason":"source acoustic profile missing"}));
                continue;
            }
        };
        if profile.foil_borne {
            continue;
        }
        let env = state.config.environment.as_ref();
        let coplanar = state.depth_motion.get(&candidate.id).is_some_and(|depth| {
            depth.tick == w.tick
                && depth.from == Some(h.initial_depth_m)
                && depth.to == h.initial_depth_m
                && candidate.depth_m == h.initial_depth_m
        });
        let scope = radial
            && body.depth_m == h.initial_depth_m
            && coplanar
            && usable == 1.
            && f.airdrop_entry
                .as_ref()
                .is_some_and(|entry| entry.resolved_at.is_some_and(|tick| tick < w.tick))
            && f.placement.as_ref().is_some_and(|p| !p.placed)
            && candidate.id != f.target
            && candidate.combat_kind() == PlatformKind::Sub
            && !profile.anechoic
            && env.is_some_and(|e| {
                !e.source.trim().is_empty() && e.layer_m.is_none() && !e.reverberant
            });
        if !scope {
            unknown = true;
            evidence.push(json!({"frame":frame,"profile":profile,"reason":"outside source-bound initial radial coplanar opportunity subset"}));
            continue;
        }
        let range = acquisition_nm(
            f.weapon.generation,
            active,
            profile.noise,
            false,
            false,
            f.weapon.shallow_water_capable,
        )?;
        let Some(fraction) = first_eligible(
            body.pos,
            body_to,
            h.heading_deg,
            candidate_from,
            candidate_to,
            range,
        ) else {
            continue;
        };
        if first_eligible(
            body_to,
            body_to,
            h.heading_deg,
            candidate_to,
            candidate_to,
            range,
        )
        .is_none()
        {
            unknown = true;
            evidence.push(json!({"frame":frame,"profile":profile,"range_nm":range,"fraction":fraction,
                "reason":"transient acquisition leaves source range/cone before epoch end; retention procedure pending"}));
            continue;
        }
        let attack_speed_index = f
            .weapon
            .settings
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.speed_kts.total_cmp(&b.speed_kts))
            .map(|(index, _)| index)
            .ok_or("7.5.7 A1 fastest source setting missing")?;
        evidence.push(json!({"frame":frame,"profile":profile,"range_nm":range,"fraction":fraction,"qualified":true}));
        qualified.push(AcquisitionRecord {
            revision: tables()["execution_revision"]["initial_acquisition_a1"]["policy_id"]
                .as_str()
                .ok_or("7.5.6 A1 execution policy missing")?
                .into(),
            original_target: f.target,
            target: candidate.id,
            owner: body.side,
            tick: w.tick,
            fraction,
            mode: if active { "active" } else { "passive" }.into(),
            position: body.pos.add(body_to.sub(body.pos).scaled(fraction)),
            target_position: candidate_from.add(candidate_to.sub(candidate_from).scaled(fraction)),
            depth_m: h.initial_depth_m,
            search_speed_index: f.speed_index,
            attack_speed_index,
            pursuit_started_at: None,
            threat: None,
            terminal: None,
        });
    }
    if unknown || qualified.len() > 1 {
        referee_trace(
            w,
            f.shooter,
            "7.5.6.acquisition_pending",
            json!({"torpedo":id,"epoch_tick":w.tick,
            "qualified_count":qualified.len(),"unknown_competitor":unknown,"candidates":evidence}),
        );
        return Err("7.5.6 A1 complete epoch contains unknown/multiple/unsupported candidates; source acquisition decision pending".into());
    }
    Ok(qualified.pop())
}

pub(super) fn advance(
    w: &mut World,
    state: &State,
    id: EntityId,
    def: &WeaponDef,
    f: &mut Flight,
) -> Result<(), String> {
    let mut record = f
        .acquisition
        .take()
        .ok_or("10.2 A1 saved acquisition missing")?;
    let result = advance_inner(w, state, id, def, f, &mut record);
    f.acquisition = Some(record);
    result
}

fn advance_inner(
    w: &mut World,
    state: &State,
    id: EntityId,
    def: &WeaponDef,
    f: &mut Flight,
    a: &mut AcquisitionRecord,
) -> Result<(), String> {
    if a.terminal.is_some() {
        return Err("10.2.7 A1 original first-attack result retained; source reattack remains pending without another die".into());
    }
    if w.tick <= a.tick {
        return Ok(());
    }
    let target = w
        .entity(a.target)
        .filter(|e| e.alive)
        .cloned()
        .ok_or("10.2 A1 captured target lost; reassignment is not inferred")?;
    let profile = target_spec(w, state, target.id)?;
    if profile.source.trim().is_empty()
        || profile.foil_borne
        || target.combat_kind() != PlatformKind::Sub
    {
        return Err(
            "10.2 A1 captured target domain/source changed; complete decision pending".into(),
        );
    }
    if target.depth_m != a.depth_m
        || !state
            .depth_motion
            .get(&target.id)
            .is_some_and(|d| d.tick == w.tick && d.from == Some(a.depth_m) && d.to == a.depth_m)
    {
        return Err(
            "10.2 A1 vertical pursuit beyond coplanar source subset remains pending".into(),
        );
    }
    if a.threat.is_none()
        && super::super::detection2026::known_category(w, target.side, id)
            == Some(PlatformKind::Torpedo)
        && w.sides[target.side.index()]
            .contacts
            .values()
            .any(|c| c.about == id && !c.stale)
    {
        // sim::tick calls begin before commands/Movement and performs sensing
        // afterwards. A direct advance without that source-order snapshot may
        // not consume the defender's first reaction as a later baseline.
        return Err("10.2.5 A1 detected defender lacks its precommand reaction baseline; source-ordered begin required, late snapshot forbidden".into());
    }
    let body = w
        .entity(id)
        .cloned()
        .ok_or("10.2 A1 torpedo body missing")?;
    let setting = f
        .weapon
        .settings
        .get(a.attack_speed_index)
        .ok_or("7.5.7 A1 fastest setting missing")?;
    if !setting.speed_kts.is_finite()
        || setting.speed_kts <= 0.
        || !setting.range_nm.is_finite()
        || setting.range_nm <= 0.
        || !f.fuel.is_finite()
        || !(0. ..=1.).contains(&f.fuel)
    {
        return Err("10.2 A1 invalid saved source endurance; complete decision pending".into());
    }
    let speed = setting.speed_kts / 3600.;
    let remaining = (1. - f.fuel) * setting.range_nm;
    let affordable = speed.min(remaining);
    let distance = body.pos.range_to(target.pos);
    let reached = distance <= affordable;
    let travel = affordable.min(distance);
    let course = body.pos.bearing_to(target.pos);
    let pos = body.pos.advanced(course, travel);
    f.speed_index = a.attack_speed_index;
    f.run_nm += travel;
    f.fuel = (f.fuel + travel / setting.range_nm).min(1.);
    a.pursuit_started_at.get_or_insert(w.tick);
    if let Some(e) = w.entity_mut(id) {
        e.pos = pos;
        e.course_deg = course;
        e.depth_m = a.depth_m;
        e.speed_kts = setting.speed_kts;
        if let Some(p) = e.projectile.as_mut() {
            p.live_s += travel / speed;
            p.enabled = a.mode == "active";
        }
    }
    if !reached {
        if remaining <= speed {
            f.fuel = 1.;
            referee_trace(
                w,
                f.shooter,
                "10.2.source_runout",
                json!({"torpedo":id,"spent_fraction":f.fuel,"acquisition":a}),
            );
            retire(w, id);
        }
        return Ok(());
    }
    let evading = a
        .threat
        .as_ref()
        .is_some_and(|threat| evasion(threat, &target, false));
    let acm = super::super::ew2026::active_acms(w, target.id)
        .iter()
        .max_by_key(|a| a.generation)
        .map(|a| a.generation as usize);
    let probability = homing_probability(f.weapon.generation, acm, evading)?;
    let d100 = rng_die(w, id, &format!("10.2.5.attack:{}", f.pass), 100);
    let hit = source_percent_hit(probability, d100);
    f.last_probability = Some(probability);
    f.last_d100 = Some(d100);
    a.terminal = Some(FirstAttack {
        tick: w.tick,
        d100,
        probability,
        hit,
        evading,
        acm_generation: acm,
    });
    referee_trace(
        w,
        f.shooter,
        "10.2.5.source_terminal",
        json!({"torpedo":id,"pass":f.pass,"target":target.id,
        "d100":d100,"probability":probability,"evading":evading,"acm_generation":acm,"hit":hit,"acquisition":a}),
    );
    if hit {
        crate::damage::apply_projectile_hit(w, id, target.id, def, true, a.owner);
        retire(w, id);
        Ok(())
    } else {
        f.pass += 1;
        f.phase = Phase::Past {
            remaining_nm: yards_nm(
                tables()["constants"]["reattack_past_yards"]
                    .as_f64()
                    .unwrap(),
            ),
        };
        Err("10.2.7 A1 first attack missed; original D10/D6/D100 and1000yd Past declaration retained; reattack remains pending".into())
    }
}

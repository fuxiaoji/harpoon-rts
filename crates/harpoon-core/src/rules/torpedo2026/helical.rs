//! §7.5.3 numeric facts plus explicitly approved H1 RTS interpolation.
//! Only initial non-wire aerial search, down and first ascent; no acquisition,
//! opportunity selection or reattack is inferred from physical motion.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HelicalState {
    pub revision: String,
    pub center: Vec2,
    pub heading_deg: f64,
    pub radial_run_nm: f64,
    pub circle_turns: f64,
    pub initial_depth_m: f64,
    pub band_coordinate: f64,
    pub ascending: bool,
    pub max_depth_m: f64,
    pub upper_boundary_pending: bool,
    pub elapsed_s: f64,
    pub endurance_nm: f64,
}

fn coordinate(depth: f64) -> Result<f64, String> {
    let limits = tables()["depth_limits_m"].as_array().unwrap();
    if !depth.is_finite() || depth < 25. || depth > 1200. {
        return Err(
            "7.5.3 H1 finite physical depth anchors required; Very Deep remains pending".into(),
        );
    }
    for i in 1..limits.len() {
        let lower = limits[i - 1].as_f64().unwrap();
        let upper = limits[i].as_f64().unwrap();
        if depth <= upper {
            return Ok((i - 1) as f64 + (depth - lower) / (upper - lower));
        }
    }
    Ok((limits.len() - 1) as f64)
}

fn depth_at(band: f64) -> f64 {
    let limits = tables()["depth_limits_m"].as_array().unwrap();
    let index = (band.floor().max(0.) as usize).min(limits.len() - 1);
    let lower = limits[index].as_f64().unwrap();
    let Some(upper) = limits.get(index + 1).and_then(Value::as_f64) else {
        return lower;
    };
    lower + (upper - lower) * (band - index as f64).clamp(0., 1.)
}

pub(super) fn initialize(w: &World, f: &Flight) -> Result<HelicalState, String> {
    let entry = f
        .airdrop_entry
        .as_ref()
        .ok_or("7.5.3 H1 actual saved aerial entry required")?;
    if f.pass != 0 || !f.weapon.guidance.acoustic() || f.weapon.guidance.wired() {
        return Err("7.5.3 H1 covers initial non-wire acoustic air entry only; full search/reattack pending".into());
    }
    if !f.order.depth_m.is_finite() || !(25. < f.order.depth_m && f.order.depth_m <= 50.) {
        return Err("7.5.3 H1 declared depth is outside supported Shallow search; source reconciliation pending".into());
    }
    if f.order.enable_run_nm != 0. {
        return Err("9.7.1 delayed enable with immediate entry remains pending".into());
    }
    if !entry.position.x.is_finite()
        || !entry.position.y.is_finite()
        || !entry.course_deg.is_finite()
    {
        return Err("7.5.3 H1 finite saved entry datum required".into());
    }
    if f.weapon.source.trim().is_empty() || f.weapon.max_depth_m <= 50. {
        return Err("7.5.3 H1 source maximum depth deeper than Shallow required".into());
    }
    coordinate(f.weapon.max_depth_m)?;
    let water = w
        .rules
        .detection
        .as_ref()
        .and_then(|s| s.config.environment.as_ref())
        .filter(|e| !e.source.trim().is_empty() && e.water_depth_m.is_finite())
        .ok_or("7.5.3 H1 source water depth required; bottom interaction remains pending")?;
    if water.water_depth_m <= f.weapon.max_depth_m {
        return Err(
            "7.5.3 H1 water must contain the source maximum depth; bottom interaction pending"
                .into(),
        );
    }
    Ok(HelicalState {
        revision: tables()["execution_revision"]["helical_h1"]["policy_id"]
            .as_str()
            .ok_or("7.5.3 H1 execution policy missing")?
            .into(),
        center: entry.position,
        heading_deg: entry.course_deg.rem_euclid(360.),
        radial_run_nm: 0.,
        circle_turns: 0.,
        initial_depth_m: f.order.depth_m,
        band_coordinate: coordinate(f.order.depth_m)?,
        ascending: false,
        max_depth_m: f.weapon.max_depth_m,
        upper_boundary_pending: false,
        elapsed_s: 0.,
        endurance_nm: 0.,
    })
}

fn point_segment_distance(point: Vec2, from: Vec2, to: Vec2) -> f64 {
    let segment = to.sub(from);
    let relative = point.sub(from);
    let squared = segment.x * segment.x + segment.y * segment.y;
    let fraction = if squared > 0. {
        ((relative.x * segment.x + relative.y * segment.y) / squared).clamp(0., 1.)
    } else {
        0.
    };
    point.range_to(from.add(segment.scaled(fraction)))
}

/// Distance to the candidate's recorded path, before expanding by our own
/// affordable motion. Missing history is not a stationary path: a new moving
/// body is bounded locally by one full tick of its known physical speed cap.
pub(super) fn candidate_distance(
    w: &World,
    state: &State,
    origin: Vec2,
    candidate: &crate::world::Entity,
) -> Result<f64, String> {
    let finite = |point: Vec2| point.x.is_finite() && point.y.is_finite();
    if !finite(candidate.pos) || !candidate.speed_kts.is_finite() || candidate.speed_kts < 0. {
        return Err("7.5.3 H1 candidate physical motion unknown; complete decision pending".into());
    }
    if let Some(motion) = state.motion.get(&candidate.id).filter(|m| m.tick == w.tick) {
        if !finite(motion.from) || !finite(motion.to) {
            return Err(
                "7.5.3 H1 saved candidate Motion invalid; complete decision pending".into(),
            );
        }
        // step() gives a newly encountered entity from==to because no prior
        // position exists. A moving spawn must retain the local uncertainty.
        if motion.from != motion.to || candidate.speed_kts == 0. {
            return Ok(point_segment_distance(origin, motion.from, motion.to)
                .min(point_segment_distance(origin, motion.to, candidate.pos)));
        }
    }
    let speed_cap = if let Some(other) = state.flights.get(&candidate.id) {
        other
            .weapon
            .settings
            .iter()
            .map(|setting| setting.speed_kts)
            .reduce(f64::max)
    } else if candidate.kind.is_projectile() {
        candidate
            .projectile
            .as_ref()
            .and_then(|projectile| w.db.weapons.get(&projectile.weapon))
            .map(|weapon| weapon.speed_kts)
    } else {
        candidate.class_def(&w.db).map(|class| class.max_speed_kts)
    }
    .filter(|speed| speed.is_finite() && *speed >= 0.)
    .ok_or("7.5.3 H1 candidate lacks saved Motion and a finite physical speed bound; complete decision pending")?;
    let prior_travel_bound = speed_cap.max(candidate.speed_kts) / 3600.;
    Ok((origin.range_to(candidate.pos) - prior_travel_bound).max(0.))
}

/// Expand the candidate's real swept segment by this weapon's affordable
/// travel. A possible interaction remains a referee decision, not acquisition.
fn possible_interaction(
    w: &World,
    state: &State,
    id: EntityId,
    _f: &Flight,
    travel_bound: f64,
) -> Result<(), String> {
    let body = w.entity(id).ok_or("7.5.3 H1 body missing")?;
    for candidate in w.entities.values().filter(|e| e.alive && e.id != id) {
        if candidate.kind == PlatformKind::Torpedo {
            if state
                .flights
                .get(&candidate.id)
                .is_none_or(|other| other.weapon.guidance.acoustic())
                && candidate_distance(w, state, body.pos, candidate)?
                    <= yards_nm(5000.) + travel_bound
            {
                return Err("7.5.3 H1 possible acoustic-torpedo interaction within5000yd; same-type/simultaneity adjudication pending".into());
            }
            continue;
        }
    }
    Ok(())
}

pub(super) fn advance(
    w: &mut World,
    state: &State,
    id: EntityId,
    f: &mut Flight,
    dt: f64,
) -> Result<(), String> {
    let mut h = f.helical.take().ok_or("7.5.3 H1 state missing")?;
    let result = advance_state(w, state, id, f, &mut h, dt);
    f.helical = Some(h);
    result
}

fn advance_state(
    w: &mut World,
    state: &State,
    id: EntityId,
    f: &mut Flight,
    h: &mut HelicalState,
    dt: f64,
) -> Result<(), String> {
    if h.upper_boundary_pending {
        return Err(
            "7.5.3 H1 first upward Shallow boundary retained; subsequent depth cycle pending"
                .into(),
        );
    }
    let setting = f
        .weapon
        .settings
        .get(f.speed_index)
        .ok_or("7.5.3 H1 source speed setting missing")?;
    let speed = setting.speed_kts / 3600.;
    let radius = yards_nm(
        tables()["constants"]["air_search_diameter_yards"]
            .as_f64()
            .unwrap(),
    ) / 2.;
    let circle_cost = yards_nm(
        tables()["constants"]["air_search_endurance_yards"]
            .as_f64()
            .unwrap(),
    );
    let bands_per_circle = tables()["execution_revision"]["helical_h1"]["bands_per_circle"]
        .as_f64()
        .unwrap();
    let circle_time = circle_cost / speed;
    let initial_band = coordinate(h.initial_depth_m)?;
    let max_band = coordinate(h.max_depth_m)?;
    if !dt.is_finite()
        || dt < 0.
        || !speed.is_finite()
        || speed <= 0.
        || !setting.range_nm.is_finite()
        || setting.range_nm <= 0.
        || !f.fuel.is_finite()
        || f.fuel < 0.
        || !h.radial_run_nm.is_finite()
        || !(0. ..=radius).contains(&h.radial_run_nm)
        || !h.circle_turns.is_finite()
        || h.circle_turns < 0.
    {
        return Err("7.5.3 H1 invalid saved physical progress; referee migration required".into());
    }
    let remaining = (1. - f.fuel).max(0.) * setting.range_nm;
    let fuel_time = remaining / speed;
    let radial_time = (radius - h.radial_run_nm).max(0.) / speed;
    let upper_turns = (2. * max_band - initial_band - 1.) / bands_per_circle;
    let upper_time = radial_time + (upper_turns - h.circle_turns).max(0.) * circle_time;
    let usable = dt.min(fuel_time).min(upper_time);
    let decision = possible_interaction(w, state, id, f, speed * usable)
        .and_then(|_| initial_acquisition::select(w, state, id, f, h, usable, speed));
    let acquisition = match decision {
        Ok(acquisition) => acquisition,
        Err(reason) => {
            referee_trace(
                w,
                f.shooter,
                "7.5.3.helical_boundary",
                json!({"torpedo":id,"reason":reason,"helical":h}),
            );
            return Err(reason);
        }
    };
    let radial_dt = usable.min(radial_time);
    let radial_distance = speed * radial_dt;
    let prior_turns = h.circle_turns;
    h.radial_run_nm = (h.radial_run_nm + radial_distance).min(radius);
    h.circle_turns = (h.circle_turns + (usable - radial_dt) / circle_time).min(upper_turns);
    h.elapsed_s += usable;
    let cost = speed * usable;
    h.endurance_nm += cost;
    f.run_nm += radial_distance + std::f64::consts::TAU * radius * (h.circle_turns - prior_turns);
    f.fuel = (f.fuel + cost / setting.range_nm).min(1.);
    let (pos, course, depth) = if h.radial_run_nm < radius {
        (
            h.center.advanced(h.heading_deg, h.radial_run_nm),
            h.heading_deg,
            h.initial_depth_m,
        )
    } else {
        let unfolding = initial_band + bands_per_circle * h.circle_turns;
        h.ascending = unfolding >= max_band;
        h.band_coordinate = if h.ascending {
            2. * max_band - unfolding
        } else {
            unfolding
        };
        h.band_coordinate = h
            .band_coordinate
            .max(if h.ascending { 1. } else { initial_band })
            .min(max_band);
        (
            h.center
                .advanced(h.heading_deg + h.circle_turns * 360., radius),
            (h.heading_deg + 90. + h.circle_turns * 360.).rem_euclid(360.),
            depth_at(h.band_coordinate).min(h.max_depth_m),
        )
    };
    if let Some(body) = w.entity_mut(id) {
        body.pos = pos;
        body.course_deg = course;
        body.depth_m = depth;
        // Rated selected speed governs source endurance/noise; the approved
        // 500yd circle clock has a shorter150pi-yard planar arc.
        body.speed_kts = setting.speed_kts;
        if let Some(projectile) = body.projectile.as_mut() {
            projectile.live_s += usable;
            projectile.enabled = f.weapon.guidance != Guidance::Passive;
        }
    }
    if let Some(acquisition) = acquisition {
        // The entire detection epoch was charged and moved at the old setting.
        // A separate private target starts fastest-setting pursuit next tick.
        f.phase = Phase::Locked;
        referee_trace(
            w,
            f.shooter,
            "7.5.7.acquired",
            json!({"torpedo":id,
            "acquisition":acquisition,"remaining_fraction":1.-f.fuel,
            "fastest_setting_effective_tick":w.tick+1}),
        );
        if let Some(projectile) = w.entity_mut(id).and_then(|body| body.projectile.as_mut()) {
            projectile.target = Some(acquisition.target);
        }
        f.acquisition = Some(acquisition);
    }
    if fuel_time <= dt && fuel_time <= upper_time {
        f.fuel = 1.;
        referee_trace(
            w,
            f.shooter,
            "10.2.source_runout",
            json!({"torpedo":id,"spent_fraction":f.fuel,"helical":h}),
        );
        retire(w, id);
        return Ok(());
    }
    if upper_time <= dt {
        h.upper_boundary_pending = true;
        referee_trace(
            w,
            f.shooter,
            "7.5.3.helical_boundary",
            json!({"torpedo":id,"helical":h,"reason":"first upward Shallow boundary; subsequent cycle pending"}),
        );
        return Err(
            "7.5.3 H1 first upward Shallow boundary; subsequent depth cycle remains pending".into(),
        );
    }
    Ok(())
}

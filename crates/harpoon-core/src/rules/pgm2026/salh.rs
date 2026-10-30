//! Source9.6.8 SALH continuation of the shared PGM registry and physical packet.
use super::*;
use crate::rules::detection2026::laser::{self, BoundDesignation, DesignationSample};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponDeclaration {
    pub source: String,
    pub guidance: String,
    pub generation: u8,
    pub min_range_nm: f64,
    pub range_nm: f64,
    pub speed_kts: f64,
    pub flight_path: String,
    pub hang_weight_kg: f64,
    pub damage_points: f64,
    pub penetration_cm: f64,
    pub signature: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit {
    pub source: String,
    pub weapon: String,
    pub rounds: u16,
    pub external: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Glide,
    Level,
    #[serde(alias = "lob", alias = "toss")]
    LobToss,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Briefing {
    pub source: String,
    pub target: String,
    pub aimpoint: String,
    pub known_kind: PlatformKind,
    pub known_motion: Motion,
    pub allowed_sides: Vec<Side>,
    pub delivery: Delivery,
    pub designator: String,
    pub device: String,
    #[serde(default)]
    pub fcs: Option<AuthoredFcs>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub weapons: BTreeMap<String, WeaponDeclaration>,
    pub fits: BTreeMap<String, Fit>,
    pub briefings: BTreeMap<String, Briefing>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundFit {
    pub fit: Fit,
    pub class: String,
    pub side: Side,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Allocation {
    pub designator: EntityId,
    pub device: String,
    pub delivery: Delivery,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Guidance {
    pub designation: BoundDesignation,
    pub delivery: Delivery,
    pub lost_los: bool,
    pub pending_reason: Option<String>,
    pub last_sample: Option<DesignationSample>,
    pub sample_count: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Arrival {
    pub at: i64,
    pub identity: String,
    pub original_d100: u8,
    pub ordinal: Option<u32>,
    pub obscured: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Smoke {
    pub target: EntityId,
    pub source_turn: i64,
    pub count: u32,
    pub first_explosion_at: i64,
    pub clear_at: i64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Option<Configuration>,
    pub fits: BTreeMap<EntityId, BoundFit>,
    pub allocations: BTreeMap<String, Allocation>,
    pub arrivals: BTreeMap<EntityId, Arrival>,
    pub smoke: BTreeMap<String, Smoke>,
    pub sampled_at: Option<i64>,
}
fn turn_seconds() -> i64 {
    tables()["salh_execution"]["tactical_turn_seconds"]
        .as_i64()
        .unwrap()
}
pub(super) fn is_weapon(weapon: &str) -> bool {
    tables()["weapons"][weapon]["guidance"]
        .as_str()
        .is_some_and(|s| s.starts_with("SALH/"))
}
fn exact_weapon(id: &str, d: &WeaponDeclaration) -> bool {
    let r = &tables()["weapons"][id];
    d.source.trim().len() > 0
        && r["guidance"] == format!("SALH/{}", d.generation)
        && (d.guidance == "SALH" || d.guidance == format!("SALH/{}", d.generation))
        && r["min_range_nm"].as_f64() == Some(d.min_range_nm)
        && r["range_nm"].as_f64() == Some(d.range_nm)
        && r["speed_kts"].as_f64() == Some(d.speed_kts)
        && r["hang_weight_kg"].as_f64() == Some(d.hang_weight_kg)
        && r["damage_points"].as_f64() == Some(d.damage_points)
        && r["penetration_cm"].as_f64() == Some(d.penetration_cm)
        && r["flight_path"] == d.flight_path
        && r["signature"] == d.signature
}
/// Extend only the existing PGM state; normal briefing, RF, budget, release
/// and physical advancement remain owned by the parent executor.
pub(super) fn configure(w: &World, parent: &mut super::State) -> Result<(), String> {
    let Some(config) = w.context.salh_2026.clone() else {
        return Ok(());
    };
    if config.source.trim().is_empty() {
        return Err("9.6.8 SALH configuration source required".into());
    }
    let mut state = State {
        config: Some(config.clone()),
        ..Default::default()
    };
    for (id, d) in &config.weapons {
        if !exact_weapon(id, d) {
            return Err(
                "9.6.8 declared SALH weapon differs from its visually verified source row".into(),
            );
        }
    }
    for (name, f) in &config.fits {
        let es: Vec<_> = w.entities.values().filter(|e| e.name == *name).collect();
        if es.len() != 1 {
            return Err("9.6.8 SALH author fit requires one exact aircraft body".into());
        }
        let e = es[0];
        if e.kind != PlatformKind::Aircraft
            || !f.source.starts_with("scenario_assumption:")
            || !f.external
            || f.rounds == 0
            || !config.weapons.contains_key(&f.weapon)
            || external_store_weight(&e.class, &f.weapon).is_none()
            || u64::from(f.rounds)
                > tables()["salh_author_external_fits"][&e.class]["weapons"][&f.weapon]
                    ["maximum_rounds"]
                    .as_u64()
                    .unwrap_or(0)
        {
            return Err("9.6.8 exact declared author SALH external fit/source or published store weight unavailable".into());
        }
        let physical: u32 = e
            .weapons
            .iter()
            .filter(|m| m.weapon == f.weapon)
            .map(|m| u32::from(m.mounts) * u32::from(m.ammo))
            .sum();
        let flight = w
            .rules
            .aviation
            .as_ref()
            .and_then(|s| s.flights.get(&e.id))
            .ok_or("9.6.8 SALH actual external load failed source initialization")?;
        if physical != u32::from(f.rounds)
            || flight.external_weapon_rounds.get(&f.weapon) != Some(&f.rounds)
        {
            return Err(
                "9.6.8 SALH physical rounds and actual external mass ledger mismatch".into(),
            );
        }
        state.fits.insert(
            e.id,
            BoundFit {
                fit: f.clone(),
                class: e.class.clone(),
                side: e.side,
            },
        );
    }
    for (key, b) in &config.briefings {
        if parent.config.briefings.contains_key(key) {
            return Err("9.6.8 SALH and existing PGM briefing keys conflict".into());
        }
        let ds: Vec<_> = w
            .entities
            .values()
            .filter(|e| e.name == b.designator)
            .collect();
        if ds.len() != 1 {
            return Err("9.6.8 SALH source designator name must resolve uniquely".into());
        }
        let d = ds[0];
        let c = w
            .rules
            .detection
            .as_ref()
            .and_then(|s| s.laser.components.get(&format!("{}:{}", d.id.0, b.device)))
            .ok_or("9.6.8 actual installed designator component absent")?;
        if c.fit.device != laser::Device::Designator || b.source.trim().is_empty() {
            return Err(
                "9.6.8 actual source Designator required; LRMTS/rangefinder is not designation"
                    .into(),
            );
        }
        let aim = w
            .context
            .aviation_2026
            .as_ref()
            .and_then(|s| s.surface_aimpoints.get(&b.aimpoint))
            .ok_or("9.6.8 allocated fixed SALH aimpoint absent")?;
        let job = w
            .rules
            .detection
            .as_ref()
            .and_then(|s| s.laser.jobs.get(&d.id))
            .ok_or("9.6.8 actual allocated designation job absent")?;
        if job.job.known_aimpoint.range_to(aim.point)
            > tables()["rts"]["fixed_point_tolerance_nm"]
                .as_f64()
                .unwrap()
        {
            return Err("9.6.8 own allocated designation job and briefing point differ".into());
        }
        state.allocations.insert(
            key.clone(),
            Allocation {
                designator: d.id,
                device: b.device.clone(),
                delivery: b.delivery,
                source: b.source.clone(),
            },
        );
        parent.config.briefings.insert(
            key.clone(),
            super::Briefing {
                source: b.source.clone(),
                target: b.target.clone(),
                aimpoint: b.aimpoint.clone(),
                known_kind: b.known_kind,
                known_motion: b.known_motion,
                moving_lead: None,
                allowed_sides: b.allowed_sides.clone(),
                fcs: b.fcs.clone(),
            },
        );
    }
    parent.salh = state;
    Ok(())
}
pub(super) fn ready(w: &World, actor: EntityId, request: &Request) -> Result<Prepared, String> {
    if !active(w) {
        return Err("9.6.8 source SALH requires explicit v4".into());
    }
    let state = w.rules.pgm.as_ref().ok_or("9.6.8 PGM state absent")?;
    let fit = state
        .salh
        .fits
        .get(&actor)
        .ok_or("9.6.8 no actual source-bound SALH mounting for this aircraft")?;
    let a = w
        .entity(actor)
        .filter(|e| {
            e.alive
                && e.kind == PlatformKind::Aircraft
                && e.class == fit.class
                && e.side == fit.side
        })
        .ok_or("9.6.8 SALH aircraft source binding changed")?;
    super::validate_platform(w, actor, &request.weapon)?;
    if request.weapon != fit.fit.weapon || request.rounds == 0 || request.rounds > fit.fit.rounds {
        return Err("9.6.8 SALH positive rounds within actual author fit required".into());
    }
    let b = state
        .config
        .briefings
        .get(&request.briefing)
        .filter(|b| b.allowed_sides.contains(&a.side))
        .ok_or("9.6.8 own allocated SALH briefing unavailable")?;
    let allocation = state
        .salh
        .allocations
        .get(&request.briefing)
        .ok_or("9.6.8 SALH designation allocation absent")?;
    let aim = av::surface_aimpoint(w, a.side, &b.aimpoint)
        .ok_or("9.6.8 own allocated fixed SALH point absent")?;
    if b.known_motion != Motion::Stationary {
        return Err(
            "9.6.8 dynamic SALH target/beam tracking is not supplied by a fixed-point laser job"
                .into(),
        );
    }
    if allocation.delivery == Delivery::LobToss {
        if allocation.designator == actor {
            return Err("9.6.8 lob/toss SALH requires a different friendly designator".into());
        }
        return Err(
            "9.6.8 lob/toss source trajectory is not supplied by this exact Glide packet fit"
                .into(),
        );
    }
    let row = &tables()["weapons"][&request.weapon];
    let minimum = row["min_launch_altitude_m"].as_f64().unwrap();
    let scale = if a.altitude_m <= 60. {
        0.25
    } else if a.altitude_m <= 2000. {
        0.5
    } else {
        1.
    };
    let distance = a.pos.range_to(aim.point);
    if !a.altitude_m.is_finite()
        || a.altitude_m <= minimum
        || !distance.is_finite()
        || distance < row["min_range_nm"].as_f64().unwrap()
        || distance > row["range_nm"].as_f64().unwrap() * scale
        || crate::geom::angle_diff(a.course_deg, a.pos.bearing_to(aim.point)).abs()
            > tables()["off_axis_deg"].as_f64().unwrap()
    {
        return Err(
            "9.6.8 source SALH release altitude/range/forward30degree envelope exceeded".into(),
        );
    }
    super::check_stock(w, actor, &request.weapon, request.rounds, true)?;
    let designation = BoundDesignation {
        designator: allocation.designator,
        device: allocation.device.clone(),
        target: state.targets[&request.briefing],
        side: a.side,
        source: allocation.source.clone(),
    };
    laser::validate_designator_control(w, &designation)?;
    Ok(Prepared {
        actor,
        side: a.side,
        class: a.class.clone(),
        request: request.clone(),
        aim: aim.point,
        target: designation.target,
        known_kind: b.known_kind,
        accepted_at: w.tick,
        identity: String::new(),
        fcs: None,
        designation: Some(designation),
    })
}
pub(super) fn guidance(w: &World, p: &Prepared) -> Option<Guidance> {
    p.designation.clone().map(|designation| Guidance {
        delivery: w.rules.pgm.as_ref().unwrap().salh.allocations[&p.request.briefing].delivery,
        designation,
        lost_los: false,
        pending_reason: None,
        last_sample: None,
        sample_count: 0,
    })
}
pub(super) fn note_release(w: &mut World, p: &Prepared) {
    if w.rules.pgm.as_ref().is_some_and(|s| {
        s.salh
            .allocations
            .get(&p.request.briefing)
            .is_some_and(|a| a.delivery == Delivery::Level)
    }) {
        let until = (w.tick.div_euclid(turn_seconds()) + 1) * turn_seconds();
        if let Some(s) = w.rules.aircombat.as_mut() {
            s.level_release_until.insert(p.actor, until);
        }
    }
}
pub(super) fn queue_arrival(w: &mut World, id: EntityId, f: &Flight) {
    let mut rng = w
        .rng
        .derive(Rng::hash_tag(f.launched_at, f.actor.0, &f.identity));
    let die = 1 + (rng.next_f64() * 100.) as u8;
    w.rules
        .pgm
        .as_mut()
        .unwrap()
        .salh
        .arrivals
        .entry(id)
        .or_insert(Arrival {
            at: w.tick,
            identity: f.identity.clone(),
            original_d100: die,
            ordinal: None,
            obscured: false,
        });
}
fn sample_flights(w: &mut World) {
    let ids: Vec<_> = w
        .rules
        .pgm
        .as_ref()
        .unwrap()
        .flights
        .iter()
        .filter(|(_, f)| f.salh.is_some())
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        if w.rules.pending_adjudications.contains_key(&id) || !w.entity(id).is_some_and(|e| e.alive)
        {
            continue;
        }
        let mut f = w.rules.pgm.as_ref().unwrap().flights[&id].clone();
        let g = f.salh.as_mut().unwrap();
        if g.last_sample.as_ref().is_some_and(|x| x.at == w.tick) {
            continue;
        }
        match laser::sample_designation(w, &g.designation) {
            Ok(sample) => {
                if !sample.active {
                    g.lost_los = true;
                }
                g.sample_count += 1;
                g.last_sample = Some(sample);
            }
            Err(reason) => {
                g.pending_reason.get_or_insert(reason);
            }
        }
        w.rules.pgm.as_mut().unwrap().flights.insert(id, f);
    }
}
/// Late current-state guidance samples see this tick's controls, movement and
/// already-applied common/nuclear/fire damage. No second FCS or global laser scan.
pub(super) fn resolve_deferred(w: &mut World) {
    if !active(w) {
        return;
    }
    sample_flights(w);
    let ids: Vec<_> = w
        .rules
        .pgm
        .as_ref()
        .unwrap()
        .salh
        .arrivals
        .keys()
        .copied()
        .filter(|id| {
            !w.rules.pending_adjudications.contains_key(id)
                && w.entity(*id).is_some_and(|e| e.alive)
        })
        .collect();
    if ids.is_empty() {
        return;
    }
    // Within a single declared salvo the original physical packet order is
    // canonical. A cross-actor simultaneous split of the two smoke-free slots
    // has no printed priority and must not choose an EntityId winner.
    let mut ambiguous = BTreeSet::new();
    let mut groups: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
    for id in &ids {
        let f = &w.rules.pgm.as_ref().unwrap().flights[id];
        groups.entry(f.target).or_default().push(*id);
    }
    for (target, group) in &groups {
        let actors: BTreeSet<_> = group
            .iter()
            .map(|id| w.rules.pgm.as_ref().unwrap().flights[id].actor)
            .collect();
        let key = format!("{}:{}", target.0, w.tick.div_euclid(turn_seconds()));
        let used = w
            .rules
            .pgm
            .as_ref()
            .unwrap()
            .salh
            .smoke
            .get(&key)
            .map_or(0, |x| x.count);
        if actors.len() > 1 && used < 2 && used + group.len() as u32 > 2 {
            ambiguous.extend(group.iter().copied());
        }
    }
    for id in ids {
        let f = w.rules.pgm.as_ref().unwrap().flights[&id].clone();
        if ambiguous.contains(&id) {
            super::hold_terminal(w,id,&f,"9.6.8 simultaneous SALH aircraft cross the two smoke-free slots; source ordering pending".into());
            continue;
        }
        let current_turn = w.tick.div_euclid(turn_seconds());
        if w.rules
            .pgm
            .as_ref()
            .unwrap()
            .salh
            .smoke
            .values()
            .any(|x| x.target == f.target && x.source_turn < current_turn && w.tick < x.clear_at)
        {
            super::hold_terminal(w,id,&f,"9.6.8 prior explosion180s smoke overlaps a new TacticalTurn; continuous boundary priority source pending".into());
            continue;
        }
        let mut arrival = w.rules.pgm.as_ref().unwrap().salh.arrivals[&id].clone();
        if arrival.ordinal.is_none() {
            let tick = w.tick;
            let key = format!("{}:{}", f.target.0, tick.div_euclid(turn_seconds()));
            let smoke = w
                .rules
                .pgm
                .as_mut()
                .unwrap()
                .salh
                .smoke
                .entry(key)
                .or_insert(Smoke {
                    target: f.target,
                    source_turn: tick.div_euclid(turn_seconds()),
                    count: 0,
                    first_explosion_at: tick,
                    clear_at: tick + turn_seconds(),
                });
            smoke.count += 1;
            arrival.ordinal = Some(smoke.count);
            arrival.obscured = smoke.count >= 3 && tick < smoke.clear_at;
            w.rules
                .pgm
                .as_mut()
                .unwrap()
                .salh
                .arrivals
                .insert(id, arrival.clone());
        }
        let g = f.salh.as_ref().unwrap();
        let result = if let Some(reason) = &g.pending_reason {
            Err(reason.clone())
        } else {
            let generation = tables()["weapons"][&f.weapon]["generation"]
                .as_u64()
                .unwrap() as u8;
            super::terminal_result(
                w,
                id,
                &f,
                ac::PgmGuidance::Salh,
                generation,
                g.lost_los || arrival.obscured,
                arrival.original_d100,
                json!({"ordinal":arrival.ordinal,"smoke_obscured":arrival.obscured,"continuity_lost":g.lost_los,"last_actual_sample":g.last_sample,"samples":g.sample_count,"late_phase":"after ordinary common damage/nuclear/fires, before logistics; explicit RTS adaptation"}),
            )
        };
        match result {
            Ok(()) => {
                w.rules.pgm.as_mut().unwrap().salh.arrivals.remove(&id);
            }
            Err(reason) => super::hold_terminal(w, id, &f, reason),
        }
    }
}

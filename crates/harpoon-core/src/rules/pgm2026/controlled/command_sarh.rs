//! §9.6.8 Command/SARH controlled leaf (English 2026-01-01 PDF108/9-7,
//! printed 9-7). Owned family beside the shared PGM executor: exact declared
//! weapon/fit, read-only ready, continuous launching-aircraft LOS sampling
//! from launch through impact via the shared referee ray plus declared author
//! smoke on the actual controller-to-target ray, original-die late-phase
//! terminal. Definite loss is sticky and quarters the existing probability
//! once; unknown ray/controller consequences and post-casualty control stay
//! pending with the original die retained.
use super::super::*;

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
    /// The printed damage cell keeps its extra D6 notation separate from
    /// ordinary damage; root copies it into the weapon damage metadata.
    pub additional_criticals: String,
    pub penetration_cm: f64,
    pub signature: String,
    /// Exact annex launch altitude when printed; null rows keep the printed
    /// general band handling (non-rail: no SSkim/NOE/VLow) in family readiness.
    pub min_launch_altitude_m: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit {
    pub source: String,
    pub weapon: String,
    pub rounds: u16,
    pub external: bool,
}

/// Author-declared obscuration of the controller-to-target ray. 9.6.8 prints
/// no Command smoke radius/TTL; these dimensions are a declared experiment
/// that exercises the printed general LOS-loss quarter.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmokeScreen {
    pub source: String,
    pub target: String,
    pub radius_nm: f64,
    /// Declared cylinder top in MSL; the shared ray helper uses 0..height.
    pub height_m: f64,
    pub obscures_controller_los_from_tick: i64,
    pub until_tick: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub weapons: BTreeMap<String, WeaponDeclaration>,
    pub fits: BTreeMap<String, Fit>,
    /// Reuses the parent briefing shape; family entries are added to
    /// `parent.config.briefings` and the parent resolves targets/aimpoints.
    pub briefings: BTreeMap<String, super::super::Briefing>,
    /// Target-name countermeasure declarations merged into the parent map so
    /// the shared terminal countermeasure resolution stays the only consumer.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub countermeasures: BTreeMap<String, super::super::Countermeasures>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub smoke_screens: BTreeMap<String, SmokeScreen>,
    /// Authored briefing to exact target radar identity for PRH.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub prh_radars: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundFit {
    pub fit: Fit,
    pub class: String,
    pub side: Side,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScreenBinding {
    pub target: EntityId,
    pub radius_nm: f64,
    pub height_m: f64,
    pub from_tick: i64,
    pub until_tick: i64,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Guidance {
    pub weapon: String,
    pub generation: u8,
    pub smoke_screen: Option<String>,
    /// Sticky definite loss: an actual failed controller-to-target ray or any
    /// declared smoke intersection, even if the screen expires before arrival.
    pub lost_los: bool,
    pub sample_count: u32,
    pub last_sample: Option<Value>,
    /// Unknown ray geometry or controller consequence; holds the original die.
    pub pending_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Arrival {
    pub at: i64,
    pub identity: String,
    pub original_d100: u8,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Option<Configuration>,
    pub fits: BTreeMap<EntityId, BoundFit>,
    pub screens: BTreeMap<String, ScreenBinding>,
    pub arrivals: BTreeMap<EntityId, Arrival>,
}

pub fn tables() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut base: Value = serde_json::from_str(include_str!("../../../../../../data/rules/command-sarh-2026.json")).expect("verified Command/SARH source data");
        let extra: Value = serde_json::from_str(include_str!("../../../../../../data/rules/pgm-remaining-2026.json")).expect("verified additional rows");
        for (id, row) in extra["weapons"].as_object().unwrap() { assert!(base["weapons"].get(id).is_none(), "duplicate Command weapon"); base["weapons"][id] = row.clone(); }
        for (id, row) in extra["author_external_fits"]["us_f18e"]["weapons"].as_object().unwrap() { assert!(base["author_external_fits"]["us_f18e"]["weapons"].get(id).is_none(), "duplicate Command fit"); base["author_external_fits"]["us_f18e"]["weapons"][id] = row.clone(); }
        base
    })
}

pub(in crate::rules::pgm2026) fn is_weapon(id: &str) -> bool {
    tables()["weapons"][id]["guidance"]
        .as_str()
        .is_some_and(|s| s.starts_with("Cmd/") || s.starts_with("PRH/"))
}

/// Author-declared external store weight for the generic aviation mass ledger.
/// Authorizes only the declared mechanism experiment, never a historical fit.
pub(in crate::rules::pgm2026) fn external_store_weight(
    class: &str,
    weapon: &str,
) -> Option<f64> {
    let t = tables();
    t["author_external_fits"][class]["weapons"][weapon]["maximum_rounds"]
        .as_u64()
        .filter(|n| *n > 0)
        .and_then(|_| t["weapons"][weapon]["hang_weight_kg"].as_f64())
}

fn exact_row(id: &str, d: &WeaponDeclaration) -> bool {
    let r = &tables()["weapons"][id];
    !d.source.trim().is_empty()
        && r["guidance"].as_str() == Some(d.guidance.as_str())
        && r["generation"].as_u64() == Some(u64::from(d.generation))
        && r["min_range_nm"].as_f64() == Some(d.min_range_nm)
        && r["range_nm"].as_f64() == Some(d.range_nm)
        && r["speed_kts"].as_f64() == Some(d.speed_kts)
        && r["flight_path"].as_str() == Some(d.flight_path.as_str())
        && r["hang_weight_kg"].as_f64() == Some(d.hang_weight_kg)
        && r["damage_points"].as_f64() == Some(d.damage_points)
        && r["additional_criticals"].as_str() == Some(d.additional_criticals.as_str())
        && r["penetration_cm"].as_f64() == Some(d.penetration_cm)
        && r["signature"].as_str() == Some(d.signature.as_str())
        && r["min_launch_altitude_m"].is_null() == d.min_launch_altitude_m.is_none()
}

/// Extend only the shared PGM state; briefing, RF placement, budget, defensive
/// phase, release, physical flight and damage stay owned by the parent executor.
pub(in crate::rules::pgm2026) fn configure(
    w: &World,
    parent: &mut super::super::State,
) -> Result<(), String> {
    let Some(config) = w.context.command_sarh_2026.clone() else {
        return Ok(());
    };
    if config.source.trim().is_empty() {
        return Err("9.6.8 Command/SARH configuration source required".into());
    }
    let mut state = State {
        config: Some(config.clone()),
        ..Default::default()
    };
    for (id, d) in &config.weapons {
        if !exact_row(id, d) {
            return Err(
                "9.6.8 declared Command/SARH weapon differs from its visually verified source row"
                    .into(),
            );
        }
        if !(1..=3).contains(&d.generation) {
            return Err("9.6.8 Command/SARH generation outside the printed PGM table rows".into());
        }
    }
    for (name, f) in &config.fits {
        let es: Vec<_> = w.entities.values().filter(|e| e.name == *name).collect();
        if es.len() != 1 {
            return Err("9.6.8 Command/SARH fit requires one exact aircraft body".into());
        }
        let e = es[0];
        if e.kind != PlatformKind::Aircraft
            || !f.source.starts_with("scenario_assumption:")
            || !f.external
            || f.rounds == 0
            || !config.weapons.contains_key(&f.weapon)
            || external_store_weight(&e.class, &f.weapon).is_none()
            || u64::from(f.rounds)
                > tables()["author_external_fits"][&e.class]["weapons"][&f.weapon]
                    ["maximum_rounds"]
                    .as_u64()
                    .unwrap_or(0)
        {
            return Err("9.6.8 exact declared author Command/SARH external fit/source or published store weight unavailable".into());
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
            .ok_or("9.6.8 Command/SARH actual external load failed source initialization")?;
        if physical != u32::from(f.rounds)
            || flight.external_weapon_rounds.get(&f.weapon) != Some(&f.rounds)
        {
            return Err(
                "9.6.8 Command/SARH physical rounds and actual external mass ledger mismatch"
                    .into(),
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
            return Err("9.6.8 Command/SARH and existing PGM briefing keys conflict".into());
        }
        if b.source.trim().is_empty() {
            return Err("9.6.8 Command/SARH briefing source required".into());
        }
        let aim = w
            .context
            .aviation_2026
            .as_ref()
            .and_then(|s| s.surface_aimpoints.get(&b.aimpoint))
            .ok_or("9.6.8 allocated fixed Command/SARH aimpoint absent")?;
        if aim.source.trim().is_empty() {
            return Err("9.6.8 Command/SARH aimpoint source required".into());
        }
        parent.config.briefings.insert(key.clone(), b.clone());
    }
    for (key, cm) in &config.countermeasures {
        if parent.config.countermeasures.contains_key(key) {
            return Err("9.6.8 Command/SARH countermeasure keys conflict with existing PGM configuration".into());
        }
        if cm.source.trim().is_empty() || cm.generation > 4 {
            return Err("9.6.8 Command/SARH countermeasure source generation0-4 required".into());
        }
        parent
            .config
            .countermeasures
            .insert(key.clone(), cm.clone());
    }
    for (key, s) in &config.smoke_screens {
        if !s.source.starts_with("scenario_assumption:") {
            return Err(
                "9.6.8 Command/SARH smoke geometry is author-declared; 9.6.8 prints no Command radius/TTL".into(),
            );
        }
        let ts: Vec<_> = w.entities.values().filter(|e| e.name == s.target).collect();
        if ts.len() != 1 {
            return Err(
                "9.6.8 Command/SARH smoke screen must resolve to one exact named target".into(),
            );
        }
        let t = ts[0];
        if !s.radius_nm.is_finite()
            || s.radius_nm <= 0.
            || !s.height_m.is_finite()
            || s.height_m <= 0.
            || s.obscures_controller_los_from_tick >= s.until_tick
        {
            return Err("9.6.8 declared Command/SARH smoke screen geometry invalid".into());
        }
        if state.screens.values().any(|b| b.target == t.id) {
            return Err("9.6.8 one target may carry only one declared Command/SARH smoke screen"
                .into());
        }
        state.screens.insert(
            key.clone(),
            ScreenBinding {
                target: t.id,
                radius_nm: s.radius_nm,
                height_m: s.height_m,
                from_tick: s.obscures_controller_los_from_tick,
                until_tick: s.until_tick,
                source: s.source.clone(),
            },
        );
    }
    parent.command_sarh = state;
    Ok(())
}

pub(in crate::rules::pgm2026) fn ready(
    w: &World,
    actor: EntityId,
    request: &Request,
) -> Result<Prepared, String> {
    if !active(w) {
        return Err("9.6.8 source Command/SARH requires explicit v4".into());
    }
    let state = w.rules.pgm.as_ref().ok_or("9.6.8 PGM state absent")?;
    let fit = state
        .command_sarh
        .fits
        .get(&actor)
        .ok_or("9.6.8 no actual source-bound Command/SARH mounting for this aircraft")?;
    let a = w
        .entity(actor)
        .filter(|e| {
            e.alive
                && e.kind == PlatformKind::Aircraft
                && e.class == fit.class
                && e.side == fit.side
        })
        .ok_or("9.6.8 Command/SARH aircraft source binding changed")?;
    super::super::validate_platform(w, actor, &request.weapon)?;
    if request.weapon != fit.fit.weapon || request.rounds == 0 || request.rounds > fit.fit.rounds {
        return Err(
            "9.6.8 Command/SARH positive rounds within actual author fit required".into(),
        );
    }
    let b = state
        .config
        .briefings
        .get(&request.briefing)
        .filter(|b| b.allowed_sides.contains(&a.side))
        .ok_or("9.6.8 own allocated Command/SARH briefing unavailable")?;
    let aim = if b.known_motion == Motion::Moving {
        w.context.aviation_2026.as_ref().and_then(|c|c.surface_aimpoints.get(&b.aimpoint))
            .filter(|p|p.allowed_sides.contains(&a.side) && !p.source.trim().is_empty() && p.controlled_range && p.point.x.is_finite() && p.point.y.is_finite())
    } else { av::surface_aimpoint(w, a.side, &b.aimpoint) }
        .ok_or("9.6.8 own allocated Command/SARH point absent")?;
    let row = &tables()["weapons"][&request.weapon];
    if b.known_motion == Motion::Moving && (b.known_kind != PlatformKind::Ship || row["flight_path"] != "Direct" || row["guidance"].as_str().is_some_and(|g|g.starts_with("PRH/"))) {
        return Err("9.6.8 moving scope requires source Direct Command against known ship".into());
    }
    let attack_point = super::super::moving_aim(w, a, b, aim.point, row["speed_kts"].as_f64().unwrap())?;
    // Printed general bands: Glide/Direct full range from High/Medium, half
    // from Low, quarter from VLow; SSkim/NOE launch stays excluded because the
    // motors ignite after release, and neither row carries a rail-launch
    // remark, so the VLow band is unreachable pending a rail-launched Cmd row.
    let scale = if a.altitude_m <= 60. {
        0.25
    } else if a.altitude_m <= 2000. {
        0.5
    } else {
        1.
    };
    let distance = a.pos.range_to(attack_point);
    if !a.altitude_m.is_finite()
        || a.altitude_m <= 60.
        || !distance.is_finite()
        || distance < row["min_range_nm"].as_f64().unwrap()
        || distance > row["range_nm"].as_f64().unwrap() * scale
        || crate::geom::angle_diff(a.course_deg, a.pos.bearing_to(attack_point)).abs()
            > super::super::tables()["off_axis_deg"].as_f64().unwrap()
    {
        return Err(
            "9.6.8 source Command/SARH release altitude/range/forward30degree envelope exceeded"
                .into(),
        );
    }
    super::super::check_stock(w, actor, &request.weapon, request.rounds, fit.fit.external)?;
    let target = *state
        .targets
        .get(&request.briefing)
        .ok_or("9.6.8 Command/SARH briefing target resolution absent from the shared registry")?;
    if tables()["weapons"][&request.weapon]["guidance"].as_str().is_some_and(|s|s.starts_with("PRH/")) {
        let sensor = state.command_sarh.config.as_ref().and_then(|c|c.prh_radars.get(&request.briefing))
            .ok_or("9.6.8 PRH allocated radar identity absent")?;
        if b.known_kind != PlatformKind::Base || !w.db.sensors.get(sensor).is_some_and(|d|d.kind == crate::db::SensorKind::Radar) {
            return Err("9.6.8 PRH allocated Base radar identity/type required".into());
        }
    }
    Ok(Prepared {
        actor,
        side: a.side,
        class: a.class.clone(),
        request: request.clone(),
        aim: attack_point,
        target,
        known_kind: b.known_kind,
        accepted_at: w.tick,
        identity: String::new(),
        fcs: None,
        designation: None,
    })
}

/// Captured at the physical commit from the family binding; plain evidence only.
pub(in crate::rules::pgm2026) fn guidance(w: &World, p: &Prepared) -> Option<Guidance> {
    let state = w.rules.pgm.as_ref()?;
    let bound = state.command_sarh.fits.get(&p.actor)?;
    Some(Guidance {
        weapon: bound.fit.weapon.clone(),
        generation: tables()["weapons"][&bound.fit.weapon]["generation"]
            .as_u64()
            .unwrap_or(0) as u8,
        smoke_screen: state
            .command_sarh
            .screens
            .iter()
            .find(|(_, s)| s.target == p.target)
            .map(|(k, _)| k.clone()),
        lost_los: false,
        sample_count: 0,
        last_sample: None,
        pending_reason: None,
    })
}

pub(in crate::rules::pgm2026) fn queue_arrival(w: &mut World, id: EntityId, f: &Flight) {
    let mut rng = w
        .rng
        .derive(Rng::hash_tag(f.launched_at, f.actor.0, &f.identity));
    let die = 1 + (rng.next_f64() * 100.) as u8;
    w.rules
        .pgm
        .as_mut()
        .unwrap()
        .command_sarh
        .arrivals
        .entry(id)
        .or_insert(Arrival {
            at: w.tick,
            identity: f.identity.clone(),
            original_d100: die,
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
        .filter(|(_, f)| f.command_sarh.is_some())
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        if w.rules.pending_adjudications.contains_key(&id) || !w.entity(id).is_some_and(|e| e.alive)
        {
            continue;
        }
        let mut f = w.rules.pgm.as_ref().unwrap().flights[&id].clone();
        if tables()["weapons"][&f.weapon]["guidance"].as_str().is_some_and(|s| s.starts_with("PRH/")) { continue; }
        let g = f.command_sarh.as_mut().unwrap();
        if g.last_sample
            .as_ref()
            .is_some_and(|x| x["at_tick"].as_i64() == Some(w.tick))
            || g.lost_los
        {
            // Sticky definite loss has already settled this flight's guidance
            // quality; later samples cannot un-lose it and an unknown after a
            // definite loss must not silently flip the printed quarter into a
            // pending hold.
            continue;
        }
        let mut sampled = false;
        // Declared author smoke obscures the CONTROLLER's view of the target:
        // the printed Command requirement is the launching aircraft's clear
        // LOS, so the actual ray starts at the aircraft body, not the bomb.
        if let Some(key) = g.smoke_screen.clone() {
            let screen = w
                .rules
                .pgm
                .as_ref()
                .unwrap()
                .command_sarh
                .screens
                .get(&key)
                .cloned();
            if let Some(s) = screen {
                if s.from_tick <= w.tick && w.tick < s.until_tick {
                    let controller = w.entity(f.actor).map(|e| (e.pos, e.altitude_m));
                    let target = w.entity(f.target).map(|e| (e.pos, e.altitude_m));
                    let center = w.entity(s.target).map(|e| e.pos);
                    if let (Some((from, from_m)), Some((to, to_m)), Some(center)) =
                        (controller, target, center)
                    {
                        match super::ray_intersects_smoke(
                            from,
                            from_m,
                            to,
                            to_m,
                            center,
                            s.radius_nm,
                            0.,
                            s.height_m,
                        ) {
                            Ok(true) => {
                                g.lost_los = true;
                                g.sample_count += 1;
                                g.last_sample = Some(json!({"at_tick":w.tick,"result":"smoke_obscured","screen":key,"policy":"sticky loss; printed general quarter applies even if the screen expires before arrival"}));
                                sampled = true;
                            }
                            Ok(false) => {}
                            Err(reason) => {
                                g.pending_reason.get_or_insert(reason);
                            }
                        }
                    }
                }
            }
        }
        // Aircraft survival alone is not LOS: the shared referee optical ray
        // decides, and its errors stay unknown instead of becoming a definite
        // loss or a definite clear.
        if !sampled && !g.lost_los {
            match super::sample_aircraft_los(w, f.actor, f.target) {
                Ok(evidence) => {
                    if evidence["clear"] == json!(false) {
                        g.lost_los = true;
                    }
                    g.sample_count += 1;
                    g.last_sample = Some(evidence);
                }
                Err(reason) => {
                    g.pending_reason.get_or_insert(reason);
                }
            }
        }
        w.rules.pgm.as_mut().unwrap().flights.insert(id, f);
    }
}

/// Late current-state samples see this tick's controls, movement and already
/// applied common damage, before physical arrivals resolve. The original
/// terminal die is settled once; no reroll and no new probability cells.
/// The single late damage batch is owned by the parent dispatch: this family
/// only samples and queues terminal results into it.
pub(in crate::rules::pgm2026) fn resolve_deferred(w: &mut World) {
    if !active(w) {
        return;
    }
    sample_flights(w);
    let ids: Vec<_> = w
        .rules
        .pgm
        .as_ref()
        .unwrap()
        .command_sarh
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
    for id in ids {
        let f = w.rules.pgm.as_ref().unwrap().flights[&id].clone();
        let arrival = w
            .rules
            .pgm
            .as_ref()
            .unwrap()
            .command_sarh
            .arrivals[&id]
            .clone();
        let g = f.command_sarh.as_ref().unwrap();
        let generation = tables()["weapons"][&f.weapon]["generation"]
            .as_u64()
            .unwrap() as u8;
        let result = if let Some(reason) = &g.pending_reason {
            Err(reason.clone())
        } else {
            super::super::terminal_result(
                w,
                id,
                &f,
                if tables()["weapons"][&f.weapon]["guidance"].as_str().is_some_and(|s| s.starts_with("PRH/")) { ac::PgmGuidance::Prh } else { ac::PgmGuidance::CommandSarh },
                generation,
                g.lost_los,
                arrival.original_d100,
                json!({"continuity_lost":g.lost_los,"smoke_screen":g.smoke_screen,"controller_samples":g.sample_count,"last_sample":g.last_sample,"declared_smoke_geometry_only":true,"scope":"stationary allocated points; moving-target tracking, control channels and post-casualty controller semantics pending"}),
            )
        };
        match result {
            Ok(()) => {
                w.rules
                    .pgm
                    .as_mut()
                    .unwrap()
                    .command_sarh
                    .arrivals
                    .remove(&id);
            }
            Err(reason) => super::super::hold_terminal(w, id, &f, reason),
        }
    }
}

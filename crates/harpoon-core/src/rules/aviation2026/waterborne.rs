//! Source §§4.6.5/4.6.6 in an explicitly declared author water/Low-flight domain.
//! A real scenario aircraft keeps its id, Flight, fuel and common loss path.
//! No water taxi, run-up duration, mooring procedure or real Annex fit inferred.
use super::{Flight, FlightSpec, Loadout, MovementLimits, Throttle};
use crate::db::PlatformClass;
use crate::types::PlatformKind;
use crate::{EntityId, Side, Vec2, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub fn tables() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/waterborne-2026.json"
        ))
        .expect("verified waterborne source and explicit author phase mapping")
    })
}
fn tolerance() -> f64 {
    tables()["rts_policy"]["numeric_equality_tolerance"]
        .as_f64()
        .unwrap()
}
fn equal(a: f64, b: f64) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= tolerance()
}
fn finite_positive(n: f64) -> bool {
    n.is_finite() && n > 0.0
}
fn source(s: &str) -> Result<(), String> {
    if s.trim().is_empty() {
        Err("4.6.5/4.6.6: explicit source/author declaration required".into())
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Configuration {
    pub source: String,
    pub cards: BTreeMap<String, Card>,
    pub units: BTreeMap<String, Binding>,
    pub environment: Environment,
    pub rts_mapping: Mapping,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Card {
    pub source: String,
    pub seaplane: bool,
    pub maximum_speed_kts: f64,
    pub fmp_low_kts: f64,
    pub low_altitude_m: f64,
    pub maneuver_light: f64,
    pub maneuver_heavy: f64,
    pub ceiling_m: f64,
    pub flight_spec: FlightSpec,
    pub load: Loadout,
    pub platform: PlatformClass,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Binding {
    pub source: String,
    pub phase: String,
    pub location: String,
    pub initial_fuel_nm: f64,
    pub mooring: Option<Mooring>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mooring {
    pub source: String,
    pub kind: String,
    pub key: String,
    pub point: Vec2,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Environment {
    pub source: String,
    pub sea_state: u8,
    pub wind_from_deg: f64,
    pub wind_speed_kts: f64,
    pub locations: BTreeMap<String, Location>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Location {
    pub source: String,
    pub shelter_states: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mapping {
    pub source: String,
    pub completion: String,
    pub resting_motion: String,
    pub airborne_scope: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub config: Configuration,
    pub units: BTreeMap<EntityId, WaterState>,
    pub pending: BTreeMap<EntityId, Procedure>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaterState {
    pub side: Side,
    pub source: String,
    pub card: String,
    pub phase: String,
    pub location: String,
    pub location_point: Vec2,
    pub mooring: Option<Mooring>,
    pub last_swamp_turn: Option<i64>,
    pub last_swamp_result: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Procedure {
    pub operation: String,
    pub accepted_at: i64,
    pub original_phase: String,
    pub card: String,
    pub source: String,
}

fn state(w: &World) -> Option<&State> {
    w.rules.aviation.as_ref()?.waterborne.as_ref()
}
fn state_mut(w: &mut World) -> Option<&mut State> {
    w.rules.aviation.as_mut()?.waterborne.as_mut()
}
fn declared_card<'a>(w: &'a World, class: &str) -> Option<&'a Card> {
    super::active(w)
        .then_some(())
        .and_then(|_| w.context.waterborne_2026.as_ref())?
        .cards
        .get(class)
}
fn validate_card(w: &World, class: &str, card: &Card) -> Result<(), String> {
    source(&card.source)?;
    source(&card.flight_spec.source)?;
    source(&card.load.source)?;
    let p =
        w.db.platforms
            .get(class)
            .ok_or("4.6.5: declared aircraft platform absent")?;
    if !card.source.starts_with("scenario_assumption:")
        || card.platform.id != class
        || p.kind != PlatformKind::Aircraft
        || card.platform.kind != PlatformKind::Aircraft
        || !equal(p.hp, card.platform.hp)
        || !equal(p.max_speed_kts, card.maximum_speed_kts)
        || !equal(card.platform.max_speed_kts, card.maximum_speed_kts)
        || !finite_positive(card.fmp_low_kts)
        || card.fmp_low_kts > card.maximum_speed_kts
        || !finite_positive(card.flight_spec.cruise_kts)
        || card.flight_spec.cruise_kts > card.fmp_low_kts
        || !finite_positive(card.flight_spec.cruise_range_nm)
        || !finite_positive(card.flight_spec.fuel_kg)
        || !finite_positive(card.flight_spec.max_payload_kg)
        || card.flight_spec.size > 3
        || !finite_positive(card.maneuver_light)
        || !finite_positive(card.maneuver_heavy)
        || !card.low_altitude_m.is_finite()
        || card.low_altitude_m <= 60.0
        || card.low_altitude_m > 2000.0
        || !card.ceiling_m.is_finite()
        || card.ceiling_m < card.low_altitude_m
        || card.ceiling_m > 2000.0
        || !equal(card.load.payload_kg, 0.0)
        || !card.load.tanks.is_empty()
        || card.load.buddy
        || !card.load.external_weapon_rounds.is_empty()
        || !card.load.internal_weapon_rounds.is_empty()
        || !p.weapons.is_empty()
        || !card.platform.weapons.is_empty()
        || !p.sensors.is_empty()
        || !card.platform.sensors.is_empty()
        || p.deck.is_some()
        || card.platform.deck.is_some()
    {
        return Err("4.6.5/4.6.6: specific pending; complete matching unarmed author Low-level card required; no real Annex, stores, higher flight or vertical profile inferred".into());
    }
    Ok(())
}

/// Creates the production Flight from the complete Scenario card. No fixture
/// Flight insertion or copying a real aircraft's sea capability is involved.
pub fn initialize(w: &mut World) -> Result<(), String> {
    if !super::active(w) || state(w).is_some() {
        return Ok(());
    }
    let Some(config) = w.context.waterborne_2026.clone() else {
        return Ok(());
    };
    source(&config.source)?;
    source(&config.environment.source)?;
    source(&config.rts_mapping.source)?;
    if config.environment.sea_state > 9
        || !config.environment.wind_from_deg.is_finite()
        || !config.environment.wind_speed_kts.is_finite()
        || config.environment.wind_speed_kts < 0.0
        || config.rts_mapping.completion != "normal_movement_end"
        || config.rts_mapping.resting_motion != "hold_author_position"
        || config.rts_mapping.airborne_scope != "Low level flight only"
    {
        return Err("4.6.5/4.6.6: specific pending; declared environment and supported author Movement-end/Low-level mapping required".into());
    }
    for location in config.environment.locations.values() {
        source(&location.source)?;
        if location.shelter_states > 2 {
            return Err(
                "4.6.6: described shelter reduction must be zero, one or two sea states".into(),
            );
        }
    }
    for (class, card) in &config.cards {
        validate_card(w, class, card)?;
    }
    let mut units = BTreeMap::new();
    let mut flights = BTreeMap::new();
    for (name, binding) in &config.units {
        source(&binding.source)?;
        let bodies:Vec<_> = w
            .entities
            .values()
            .filter(|e| e.name == *name && e.alive && e.kind == PlatformKind::Aircraft)
            .collect();
        if bodies.len()!=1{return Err("4.6.5/4.6.6: declared name must identify exactly one live aircraft; ambiguous or absent binding".into());}
        let e=bodies[0];
        let card = config
            .cards
            .get(&e.class)
            .ok_or("4.6.5/4.6.6: declared unit has no matching complete card")?;
        let location = config
            .environment
            .locations
            .get(&binding.location)
            .ok_or("4.6.6: explicit scenario location/shelter declaration absent")?;
        if !binding.initial_fuel_nm.is_finite()
            || binding.initial_fuel_nm <= 0.0
            || binding.initial_fuel_nm > card.flight_spec.cruise_range_nm
            || !matches!(binding.phase.as_str(), "waterborne" | "airborne")
            || (binding.phase == "waterborne"
                && (!equal(e.altitude_m, 0.0) || !equal(e.speed_kts, 0.0)))
            || (binding.phase == "airborne" && !equal(e.altitude_m, card.low_altitude_m))
            || (binding.phase == "airborne" && binding.mooring.is_some())
        {
            return Err(
                "4.6.5/4.6.6: declared real body phase/fuel inconsistent with author card".into(),
            );
        }
        if let Some(mooring) = &binding.mooring {
            source(&mooring.source)?;
            if !matches!(mooring.kind.as_str(), "buoy" | "pier")
                || mooring.key.trim().is_empty()
                || !mooring.point.x.is_finite()
                || !mooring.point.y.is_finite()
                || e.pos.range_to(mooring.point) > tolerance()
            {
                return Err("4.6.6: specific pending; explicit initial fixed buoy/pier mooring at the actual body required; moving tender/winch procedure unresolved".into());
            }
        }
        let _ = location;
        units.insert(
            e.id,
            WaterState {
                side: e.side,
                source: binding.source.clone(),
                card: e.class.clone(),
                phase: binding.phase.clone(),
                location: binding.location.clone(),
                location_point: e.pos,
                mooring: binding.mooring.clone(),
                last_swamp_turn: None,
                last_swamp_result: None,
            },
        );
        flights.insert(
            e.id,
            Flight {
                spec: card.flight_spec.clone(),
                fuel_nm: binding.initial_fuel_nm,
                capacity_nm: card.flight_spec.cruise_range_nm,
                throttle: Throttle::Cruise,
                payload_fraction: 0.0,
                last_tick: w.tick,
                last_position: e.pos,
                external_weapon_rounds: BTreeMap::new(),
                internal_weapon_rounds: BTreeMap::new(),
            },
        );
    }
    let aviation = w
        .rules
        .aviation
        .as_mut()
        .ok_or("4.6.5/4.6.6: ordinary aviation State must initialize first")?;
    for id in flights.keys() {
        if aviation.flights.contains_key(id) {
            return Err(
                "4.6.5/4.6.6: duplicate declared Flight cannot replace existing fuel/load state"
                    .into(),
            );
        }
    }
    aviation.flights.extend(flights);
    aviation.waterborne = Some(State {
        config,
        units,
        pending: BTreeMap::new(),
    });
    Ok(())
}

pub fn declared_flight_spec(w: &World, class: &str) -> Option<Result<FlightSpec, String>> {
    let card = declared_card(w, class)?;
    Some(validate_card(w, class, card).map(|_| card.flight_spec.clone()))
}

pub fn declared_speed(
    w: &World,
    class: &str,
    altitude: f64,
    setting: Throttle,
) -> Option<Result<f64, String>> {
    let card = declared_card(w, class)?;
    Some((|| {
        if !equal(altitude, card.low_altitude_m) {
            return Err("4.6.5: specific pending; author card supplies only its declared Low level, no vertical/higher profile".into());
        }
        match setting {
            Throttle::Cruise => Ok(card.flight_spec.cruise_kts),
            Throttle::Fmp => Ok(card.fmp_low_kts),
            _ => Err(
                "4.6.5: specific pending; author card has no declared reheat/loiter/hover row"
                    .into(),
            ),
        }
    })())
}

pub fn controls_water_motion(w: &World, unit: EntityId) -> bool {
    state(w)
        .and_then(|s| s.units.get(&unit))
        .is_some_and(|r| r.phase == "waterborne")
}

/// A bound author body uses the real shared integration; no guessed climb or
/// alternative trajectory is introduced. Ordinary undeclared classes fall on.
pub fn movement_limits(w: &World, unit: EntityId) -> Option<Result<MovementLimits, String>> {
    let s = state(w)?;
    let row = s.units.get(&unit)?;
    Some((|| {
        let e = w.entity(unit).ok_or("4.6.5: declared aircraft absent")?;
        let card = s.config.cards.get(&row.card).ok_or("4.6.5: card absent")?;
        let (minimum, maximum) = if row.phase == "waterborne" {
            (0.0, 0.0)
        } else if row.phase == "airborne" {
            if !equal(e.altitude_m, card.low_altitude_m)
                || !equal(e.ordered_altitude_m, card.low_altitude_m)
            {
                return Err("4.6.5: specific pending; only declared Low level flight is supported, no invented climb/descent".into());
            }
            let throttle = w
                .rules
                .aviation
                .as_ref()
                .and_then(|s| s.flights.get(&unit))
                .ok_or("4.6.5: actual declared Flight absent")?
                .throttle;
            let maximum = declared_speed(w, &e.class, e.altitude_m, throttle)
                .ok_or("4.6.5: actual declared speed card absent")??;
            (card.fmp_low_kts * 0.25, maximum)
        } else {
            return Err("4.6.5: specific pending; unknown saved water/air phase".into());
        };
        Ok(MovementLimits {
            climb_mps: 0.0,
            descend_mps: 0.0,
            minimum_kts: minimum,
            maximum_kts: maximum,
            ceiling_m: card.ceiling_m,
            no_acceleration: false,
        })
    })())
}

fn preflight(w: &World, unit: EntityId, operation: &str) -> Result<(), String> {
    if !super::active(w) || !matches!(operation, "takeoff" | "land") {
        return Err("4.6.5: explicit v4 water operation required".into());
    }
    let s = state(w).ok_or("4.6.5: water operation declaration absent")?;
    let row = s
        .units
        .get(&unit)
        .ok_or("4.6.5: own declared aircraft unavailable")?;
    let e = w
        .entity(unit)
        .filter(|e| {
            e.alive && e.side == row.side && e.kind == PlatformKind::Aircraft && e.class == row.card
        })
        .ok_or("4.6.5: own declared aircraft unavailable")?;
    let card = s.config.cards.get(&row.card).ok_or("4.6.5: card absent")?;
    if !card.seaplane {
        return Err("4.6.5: actual declared seaplane capability required".into());
    }
    if let Some(old) = s.pending.get(&unit) {
        if old.operation != operation {
            return Err("4.6.5: another water procedure is pending".into());
        }
    }
    let location = s
        .config
        .environment
        .locations
        .get(&row.location)
        .ok_or("4.6.5: source scenario location absent")?;
    if location.shelter_states > 0 && e.pos.range_to(row.location_point) > tolerance() {
        return Err("4.6.5: specific pending; no spatial shelter extent supplied beyond declared initial water location".into());
    }
    let effective_sea = s
        .config
        .environment
        .sea_state
        .saturating_sub(location.shelter_states);
    if effective_sea > 3 {
        return Err("4.6.5: specific pending; above safe water sea state3 unsafe-ops risk has not been source-adjudicated".into());
    }
    if s.config.environment.wind_speed_kts <= 0.0 {
        return Err(
            "4.6.5: specific pending; calm-wind takeoff/landing direction unresolved".into(),
        );
    }
    if crate::geom::angle_diff(e.course_deg, s.config.environment.wind_from_deg).abs() > tolerance()
    {
        return Err("4.6.5: actual seaplane must be heading into the declared wind".into());
    }
    if operation == "takeoff" {
        if row.phase != "waterborne" || !equal(e.altitude_m, 0.0) || !equal(e.speed_kts, 0.0) {
            return Err("4.6.5: real resting sea-surface body required for takeoff".into());
        }
        if row.mooring.is_some() {
            return Err(
                "4.6.5: specific pending; explicit unmooring procedure required before takeoff"
                    .into(),
            );
        }
    } else if row.phase != "airborne"
        || !equal(e.altitude_m, card.low_altitude_m)
        || !equal(e.speed_kts, card.maximum_speed_kts * 0.25)
    {
        return Err("4.6.5: actual Low altitude and25% maximum speed required for water landing;25%FMP is not this landing base".into());
    }
    if !w
        .rules
        .aviation
        .as_ref()
        .is_some_and(|a| a.flights.get(&unit).is_some_and(|f| f.fuel_nm > 0.0))
    {
        return Err("4.6.5: real declared Flight with remaining fuel required".into());
    }
    Ok(())
}

pub fn validate(w: &World, unit: EntityId, operation: &str) -> Result<(), String> {
    preflight(w, unit, operation)
}
pub fn declare(w: &mut World, unit: EntityId, operation: &str) -> Result<(), String> {
    preflight(w, unit, operation)?;
    let now = w.tick;
    let s = state_mut(w).ok_or("4.6.5: State absent")?;
    if s.pending.contains_key(&unit) {
        return Ok(());
    }
    let row = s.units.get(&unit).ok_or("4.6.5: aircraft absent")?;
    s.pending.insert(
        unit,
        Procedure {
            operation: operation.into(),
            accepted_at: now,
            original_phase: row.phase.clone(),
            card: row.card.clone(),
            source: s.config.rts_mapping.source.clone(),
        },
    );
    Ok(())
}

pub fn after_movement(w: &mut World) {
    let Some(s) = state(w) else {
        return;
    };
    let jobs: Vec<_> = s.pending.iter().map(|(id, p)| (*id, p.clone())).collect();
    for (unit, job) in jobs {
        let result = preflight(w, unit, &job.operation).and_then(|_| {
            let row = &state(w).ok_or("4.6.5: State absent")?.units[&unit];
            if row.phase != job.original_phase || row.card != job.card {
                return Err(
                    "4.6.5: declared original body phase/card changed before Movement end".into(),
                );
            }
            if job.operation == "land" {
                super::fuel_step(w, unit)?;
            }
            Ok(())
        });
        if let Some(s) = state_mut(w) {
            s.pending.remove(&unit);
        }
        if let Err(reason) = result {
            super::trace(
                w,
                unit,
                "4.6.5.water_cancelled",
                json!({"operation":job.operation,"reason":reason,"accepted_at":job.accepted_at}),
            );
            continue;
        }
        let Some(s) = state(w) else {
            continue;
        };
        let card = s.config.cards[&job.card].clone();
        let wind = s.config.environment.wind_from_deg;
        let now = w.tick;
        let takeoff = job.operation == "takeoff";
        let (pos, speed, altitude) = {
            let Some(e) = w.entity_mut(unit) else {
                continue;
            };
            e.speed_kts = if takeoff {
                card.fmp_low_kts * 0.25
            } else {
                0.0
            };
            e.ordered_speed_kts = e.speed_kts;
            e.altitude_m = if takeoff { card.low_altitude_m } else { 0.0 };
            e.ordered_altitude_m = e.altitude_m;
            e.course_deg = crate::geom::normalize_deg(wind);
            e.ordered_course_deg = e.course_deg;
            (e.pos, e.speed_kts, e.altitude_m)
        };
        if let Some(f) = w
            .rules
            .aviation
            .as_mut()
            .and_then(|a| a.flights.get_mut(&unit))
        {
            f.last_tick = now;
            f.last_position = pos;
        }
        if let Some(row) = state_mut(w).and_then(|s| s.units.get_mut(&unit)) {
            row.phase = if takeoff { "airborne" } else { "waterborne" }.into();
            if !takeoff {
                row.location_point = pos;
            }
        }
        super::trace(
            w,
            unit,
            if takeoff {
                "4.6.5.water_takeoff"
            } else {
                "4.6.5.water_land"
            },
            json!({"same_body":unit,"card":job.card,"accepted_at":job.accepted_at,"completed_at":now,"pos":pos,"speed_kts":speed,"altitude_m":altitude,"speed_basis":if takeoff {"fmp"}else{"maximum"},"basis_speed_kts":if takeoff {card.fmp_low_kts}else{card.maximum_speed_kts},"wind_from_deg":wind,"rts_mapping":"Explicit author normal Movement-end barrier, no printed elapsed takeoff/landing seconds inferred","source_pdf":36}),
        );
    }
}

pub fn swamp_step(w: &mut World) {
    if !super::active(w) || w.tick <= 0 || w.tick % 180 != 0 {
        return;
    }
    let Some(s) = state(w) else {
        return;
    };
    let ids: Vec<_> = s.units.keys().copied().collect();
    let turn = w.tick / 180;
    for unit in ids {
        let Some(s) = state(w) else {
            continue;
        };
        let row = s.units[&unit].clone();
        if row.phase != "waterborne" || row.last_swamp_turn.is_some_and(|last| last >= turn) {
            continue;
        }
        let Some(e) = w
            .entity(unit)
            .filter(|e| e.alive && e.kind == PlatformKind::Aircraft && e.side == row.side)
        else {
            continue;
        };
        let Some(location) = s.config.environment.locations.get(&row.location) else {
            continue;
        };
        let sea = s.config.environment.sea_state;
        let shelter = location.shelter_states;
        if !equal(e.altitude_m, 0.0)
            || !equal(e.speed_kts, 0.0)
            || (shelter > 0 && e.pos.range_to(row.location_point) > tolerance())
        {
            crate::rules::operations::blocked_once(w,unit,"4.6.6.swamp_pending","specific pending: actual resting body/location differs from declared sea surface/shelter geometry");
            continue;
        }
        let moored = if let Some(m) = &row.mooring {
            if e.pos.range_to(m.point) > tolerance() {
                crate::rules::operations::blocked_once(
                    w,
                    unit,
                    "4.6.6.swamp_pending",
                    "specific pending: actual moored body no longer at explicit fixed mooring",
                );
                continue;
            }
            true
        } else {
            false
        };
        let chance = match super::swamp_chance(sea, moored, shelter) {
            Ok(p) => p,
            Err(reason) => {
                crate::rules::operations::blocked_once(w, unit, "4.6.6.swamp_pending", &reason);
                continue;
            }
        };
        let roll = (!moored && chance > 0.0).then(|| w.scope_rng(unit, "4.6.6.swamp").next_f64());
        let lost = roll.is_some_and(|r| r < chance);
        let detail = json!({"turn":turn,"sea_state":sea,"shelter_states":shelter,"effective_sea_state":sea.saturating_sub(shelter),"chance":chance,"roll_uniform":roll,"lost":lost,"moored":moored,"environment_source":s.config.environment.source,"location_source":location.source,"body_source":row.source,"source_pdf":36});
        if let Some(row) = state_mut(w).and_then(|s| s.units.get_mut(&unit)) {
            row.last_swamp_turn = Some(turn);
            row.last_swamp_result = Some(detail.clone());
        }
        if roll.is_some() {
            super::trace(w, unit, "4.6.6.swamp", detail);
        }
        if lost {
            crate::rules::operations::crash_aircraft(w, unit);
        }
    }
}

pub fn own_view(w: &World, unit: EntityId) -> Option<Value> {
    let row = state(w)?.units.get(&unit)?;
    Some(
        json!({"phase":row.phase,"source":row.source,"card":row.card,"location":row.location,"moored":row.mooring.is_some(),"mooring":row.mooring,"last_swamp_turn":row.last_swamp_turn,"last_swamp_result":row.last_swamp_result,"pending":state(w)?.pending.get(&unit),"applicability":"explicit author Low-level/water-resting domain, not real Annex seaplane fit"}),
    )
}

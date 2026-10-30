//! Chapter3 source speed/weather/underway procedures. Explicit per-unit fits.
use crate::{EntityId, EventKind, Rng, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};
pub mod astern;
pub mod crashback;
pub mod hard;
pub mod submarine;
pub mod turning;
pub mod hazards;

pub fn tables() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| {
        let mut data:Value=serde_json::from_str(include_str!("../../../../data/rules/navigation-2026.json")).unwrap();
        data["hazards"]=serde_json::from_str(include_str!("../../../../data/rules/navigation-hazards-2026.json")).expect("source collision and grounding data");
        data
    })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Configuration {
    pub source: String,
    #[serde(default)]
    pub units: BTreeMap<String, Fit>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub hazards: Option<hazards::Configuration>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Propulsion {
    NonSteam,
    Steam,
    Nuclear,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Readiness {
    Underway,
    Anchored,
    ColdIron,
    InportSteam,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fit {
    pub source: String,
    pub size: char,
    pub merchant: bool,
    pub passenger_liner: bool,
    pub cpp: bool,
    pub waterjet: bool,
    pub undamaged_max_kts: f64,
    pub propulsion: Propulsion,
    pub readiness: Readiness,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submarine: Option<submarine::Fit>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub units: BTreeMap<EntityId, Unit>,
    pub diagnostics: BTreeSet<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub hazards: Option<hazards::State>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unit {
    pub fit: Fit,
    pub readiness: Readiness,
    pub ready_at: Option<i64>,
    pub coasting: bool,
    pub quiet: bool,
    pub heave_to: bool,
    pub window: i64,
    pub turn_degrees: f64,
    pub prior_course: f64,
    pub order_sequence: u64,
    pub underway_dice: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_motion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_operation: Option<submarine::Operation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turning: Option<turning::State>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub astern: Option<astern::State>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crash_back: Option<crashback::State>,
    #[serde(default, skip_serializing_if = "hard::HardRudder::is_absent")]
    pub hard_rudder: hard::HardRudder,
}
/// Explicit turning side for hard rudder; resolves180° and long-way turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnSide {
    Port,
    Starboard,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Beach {},
    RefloatBeached {},
    Ram {contact:crate::ContactId,#[serde(default)]speed_kts:Option<f64>},
    CancelRam {},
    Battery { action: super::battery2026::Action },
    Underway,
    EmergencyBlow { depth_m: f64 },
    /// Nonnegative ordered speed; direction is a separate source navigation state.
    Astern {
        enabled: bool,
    },
    CrashBack {enabled:bool},
    Coasting {
        enabled: bool,
    },
    Quiet {
        enabled: bool,
    },
    HeaveTo {
        enabled: bool,
    },
    HardRudder {
        course_deg: f64,
        direction: TurnSide,
    },
    RudderRecovery {
        automatic: bool,
    },
    FreeRudder {},
}
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub minimum_kts: f64,
    pub maximum_kts: f64,
    pub acceleration_per_s: f64,
    pub deceleration_per_s: f64,
    pub wind_course: Option<f64>,
    pub target_kts: Option<f64>,
}
pub fn active(w: &World) -> bool {
    w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full
}
pub fn configured(w: &World, id: EntityId) -> bool {
    active(w)
        && w.rules
            .navigation
            .as_ref()
            .is_some_and(|s| s.units.contains_key(&id))
}
fn size_index(size: char) -> Result<usize, String> {
    "ABCDEFG"
        .chars()
        .position(|c| c == size)
        .ok_or("3 source ship size must be A-G".into())
}
pub fn acceleration(f: &Fit, speed: f64) -> Result<[f64; 3], String> {
    size_index(f.size)?;
    if !speed.is_finite()
        || speed < 0.
        || !f.undamaged_max_kts.is_finite()
        || f.undamaged_max_kts <= 0.
    {
        return Err("3.1 invalid source speed".into());
    }
    let fast = f.undamaged_max_kts
        >= tables()["acceleration"]["fast_threshold_kts"]
            .as_f64()
            .unwrap();
    let category = if f.merchant && !f.passenger_liner {
        "merchant"
    } else {
        "warship"
    };
    let key = if category == "merchant" {
        match f.size {
            'A' => "a",
            'B' => "b",
            'C' | 'D' | 'E' => "c_e",
            _ => "fg",
        }
    } else {
        match f.size {
            'A' => {
                if fast {
                    "fast_a"
                } else {
                    "slow_a"
                }
            }
            'B' => {
                if f.cpp || f.waterjet {
                    "b_cpp"
                } else {
                    "b"
                }
            }
            'C' | 'D' | 'E' => {
                if f.cpp || f.waterjet {
                    "c_e_cpp"
                } else {
                    "c_e"
                }
            }
            _ => {
                if fast || f.waterjet {
                    "fast_fg"
                } else {
                    "slow_fg"
                }
            }
        }
    };
    let row = &tables()["acceleration"][category][key];
    Ok([
        row[0].as_f64().unwrap(),
        row[1].as_f64().unwrap(),
        row[2].as_f64().unwrap(),
    ])
}
pub fn weather_fraction(
    size: char,
    sea: u8,
    submarine: bool,
    depth: f64,
) -> Result<Option<f64>, String> {
    let mut index = size_index(size)?;
    if !depth.is_finite() || depth < 0. || sea > 9 {
        return Err("3.1.5 invalid sea state/depth".into());
    }
    if submarine && depth > 0. {
        return Ok(Some(1.));
    }
    if submarine {
        index = index.saturating_sub(1);
    }
    let col = match index {
        0 => 0,
        1 => 1,
        2 => 2,
        3 | 4 => 3,
        _ => 4,
    };
    Ok(tables()["sea_state_speed"][sea as usize][col].as_f64())
}
pub fn underway_seconds(f: &Fit, dice: &[u8]) -> Result<i64, String> {
    size_index(f.size)?;
    let steam = f.propulsion != Propulsion::NonSteam;
    let key = match f.size {
        'A' | 'B' => "a_b",
        'C' | 'D' => "c_d",
        'E' => "e",
        _ => "f_g",
    };
    let row = &tables()["underway"][if steam { "steam" } else { "nonsteam" }][key];
    let expression = row[0]
        .as_str()
        .ok_or("3.6 no printed steam F/G time; scenario adjudication required")?;
    let (count, sides) = match expression {
        "d10" => (1, 10),
        "d6" => (1, 6),
        "2d6" => (2, 6),
        _ => return Err("3.6 unsupported source dice".into()),
    };
    if dice.len() != count || dice.iter().any(|d| *d == 0 || *d > sides) {
        return Err("3.6 invalid original dice".into());
    }
    if f.readiness == Readiness::InportSteam && !steam {
        return Err("3.6 inport steaming needs steam/nuclear propulsion".into());
    }
    let nuclear = if f.propulsion == Propulsion::Nuclear {
        tables()["underway"]["nuclear_multiplier"].as_f64().unwrap()
    } else {
        1.
    };
    let inport = if f.readiness == Readiness::InportSteam {
        tables()["underway"]["inport_multiplier"].as_f64().unwrap()
    } else {
        1.
    };
    Ok((dice.iter().map(|d| *d as u32).sum::<u32>() as f64
        * row[1].as_f64().unwrap()
        * nuclear
        * inport) as i64)
}
fn trace(w: &mut World, id: EntityId, rule: &str, value: Value) {
    let side = w.entity(id).map(|e| e.side);
    w.emit(
        EventKind::RulesEvent {
            domain: "navigation2026".into(),
            rule: rule.into(),
            side,
            unit: Some(id),
            contact: None,
            detail: value.to_string(),
        },
        side.map(Visibility::Side).unwrap_or(Visibility::Referee),
    );
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let mut state = State::default();
    for e in w.entities.values().filter(|e|e.kind==crate::types::PlatformKind::Sub) {
        if let Some(maximum)=super::submarine_annex::verified_maximum_depth_m(w,&e.class) {
            if !e.depth_m.is_finite() || e.depth_m<0. || e.depth_m>maximum {
                return Err("3.2.1 initial depth exceeds verified Annex game band; no silent depth correction".into());
            }
            if e.depth_m==0. && super::submarine_annex::surface_max_kts(w,e).is_some_and(|limit|e.speed_kts>limit) {
                return Err("3.2.1 initial surfaced speed exceeds verified Annex surface capability".into());
            }
        }
    }
    if let Some(config) = &w.context.navigation_2026 {
        if config.source.trim().is_empty() {
            return Err("3 navigation configuration needs source/assumption label".into());
        }
        if w.context
            .radar_environment
            .as_ref()
            .is_none_or(|e| !e.valid())
        {
            return Err("3.1.5 navigation fit requires explicit source sea state".into());
        }
        for (name, fit) in &config.units {
            if fit.source.trim().is_empty() {
                return Err("3 propulsion/size fit needs source/assumption label".into());
            }
            let matching:Vec<_> = w
                .entities
                .values()
                .filter(|e| &e.name == name).collect();
            if matching.len()!=1{return Err("3 exact navigation fit must bind one unique scenario entity".into())}
            let e=matching[0];
            if !matches!(
                e.kind,
                crate::types::PlatformKind::Ship | crate::types::PlatformKind::Sub
            ) {
                return Err("3 navigation fit requires ship/submarine".into());
            }
            acceleration(fit, e.speed_kts)?;
            if let Some(sub) = &fit.submarine {
                submarine::validate_fit(w, e, sub)?;
            }
            if fit.readiness == Readiness::InportSteam && fit.propulsion == Propulsion::NonSteam {
                return Err("3.6 inport steaming is not a non-steam readiness state".into());
            }
            if fit.undamaged_max_kts > e.class_def(&w.db).ok_or("missing platform")?.max_speed_kts {
                return Err("3 authored fit cannot raise database maximum speed".into());
            }
            let cap = match fit.readiness {
                Readiness::ColdIron => 0.,
                Readiness::InportSteam => {
                    fit.undamaged_max_kts
                        * tables()["underway"]["inport_speed_fraction"]
                            .as_f64()
                            .unwrap()
                }
                _ => fit.undamaged_max_kts,
            };
            if e.speed_kts > cap {
                return Err("3.6 initial speed exceeds declared readiness".into());
            }
            state.units.insert(
                e.id,
                Unit {
                    fit: fit.clone(),
                    readiness: fit.readiness,
                    ready_at: None,
                    coasting: false,
                    quiet: true,
                    heave_to: false,
                    window: 0,
                    turn_degrees: 0.,
                    prior_course: e.course_deg,
                    order_sequence: 0,
                    underway_dice: vec![],
                    last_motion: None,
                    depth_operation: None,
                    turning: None,
                    astern: None,
                    crash_back: None,
                    hard_rudder: hard::HardRudder::default(),
                },
            );
        }
    }
    if let Some(config)=w.context.navigation_2026.as_ref().and_then(|c|c.hazards.clone()) {
        state.hazards=Some(hazards::initialize(w,&state,config)?);
    }
    w.rules.navigation = Some(state);
    Ok(())
}
pub fn validate_action(w: &World, id: EntityId, action: &Action) -> Result<(), String> {
    if matches!(action,Action::Beach{}) {return super::beached2026::validate_beach(w,id)}
    if matches!(action,Action::RefloatBeached{}) {return super::beached2026::validate_refloat(w,id,false)}
    if let Action::Ram{contact,speed_kts}=action {return hazards::validate_ram(w,id,*contact,*speed_kts)}
    if matches!(action,Action::CancelRam{}) && w.rules.navigation.as_ref().and_then(|n|n.hazards.as_ref()).is_none() {return Err("3.3 explicit collision configuration required".into())}
    if let Action::Battery { action } = action {
        let side = w.entity(id).ok_or("3.2 battery unit missing")?.side;
        return super::battery2026::validate_action(w, side, id, action);
    }
    if let Action::EmergencyBlow {depth_m}=action {
        return submarine::validate_emergency_blow(w,id,*depth_m);
    }
    if let Action::HardRudder { course_deg, .. } = action {
        return hard::validate_order(w, id, *course_deg);
    }
    if matches!(action, Action::RudderRecovery { .. } | Action::FreeRudder {}) {
        return hard::validate_recovery(w, id);
    }
    if submarine::emergency_blowing(w,id) && matches!(action,
        Action::Quiet{enabled:true}|Action::Coasting{enabled:true}|Action::HeaveTo{enabled:true}
        |Action::Astern{enabled:true}|Action::CrashBack{enabled:true}) {
        return Err("3.2.1 end Emergency Blow before conflicting navigation maneuver; combined procedure pending".into());
    }
    Ok(())
}
pub fn apply(w: &mut World, id: EntityId, action: &Action) -> Result<(), String> {
    if matches!(action,Action::Beach{}) {super::beached2026::validate_beach(w,id)?;return super::beached2026::mark(w,id,None)}
    if matches!(action,Action::RefloatBeached{}) {return super::beached2026::refloat(w,id,false)}
    if let Action::Battery { action } = action {
        let side = w.entity(id).ok_or("3.2 battery unit missing")?.side;
        return super::battery2026::apply(w, side, id, action);
    }
    if !configured(w, id) {
        return Err("3 source navigation unit fit required (v4)".into());
    }
    validate_action(w,id,action)?;
    let e = w
        .entity(id)
        .filter(|e| e.alive)
        .ok_or("3 navigation unit unavailable")?
        .clone();
    match action {
        Action::Beach{} | Action::RefloatBeached{} => unreachable!("beaching handled before navigation fit"),
        Action::Ram{contact,speed_kts}=>hazards::order_ram(w,id,*contact,*speed_kts)?,
        Action::CancelRam{}=>hazards::cancel_ram(w,id),
        Action::Battery { .. } => unreachable!("battery action is handled before navigation fit validation"),
        Action::EmergencyBlow {depth_m}=>submarine::order_emergency_blow(w,id,*depth_m)?,
        Action::CrashBack {enabled}=>crashback::order(w,id,*enabled)?,
        Action::HardRudder { course_deg, direction } => hard::order(w, id, *course_deg, *direction)?,
        Action::RudderRecovery { automatic } => hard::set_recovery(w, id, *automatic)?,
        Action::FreeRudder {} => hard::arm_free(w, id)?,
        Action::Astern { enabled } => {
            if *enabled && crashback::active(w,id){return Err("3.1.2/3.1.3 explicitly end crash back before sustained astern".into());}
            astern::order(w, id, *enabled)?;
        }
        Action::Underway => {
            let u = w.rules.navigation.as_ref().unwrap().units[&id].clone();
            if matches!(u.readiness, Readiness::Underway | Readiness::Anchored)
                || u.ready_at.is_some()
            {
                return Err("3.6 already ready or warming up".into());
            }
            let (count, sides) = if u.fit.propulsion == Propulsion::NonSteam {
                (
                    1,
                    if matches!(u.fit.size, 'F' | 'G') {
                        6
                    } else {
                        10
                    },
                )
            } else {
                (2, 6)
            };
            let mut r = w.rng.derive(Rng::hash_tag(
                w.tick,
                id.0,
                &format!("3.6.underway:{}", u.order_sequence),
            ));
            let dice: Vec<u8> = (0..count)
                .map(|_| 1 + (r.next_f64() * sides as f64) as u8)
                .collect();
            let seconds = underway_seconds(&u.fit, &dice)?;
            let due = w.tick + seconds;
            let unit = w
                .rules
                .navigation
                .as_mut()
                .unwrap()
                .units
                .get_mut(&id)
                .unwrap();
            unit.ready_at = Some(due);
            unit.underway_dice = dice.clone();
            unit.order_sequence += 1;
            trace(
                w,
                id,
                "3.6.underway_started",
                json!({"dice":dice,"seconds":seconds,"ready_at":due,"source":u.fit.source}),
            );
        }
        Action::Coasting { enabled } => {
            w.rules
                .navigation
                .as_mut()
                .unwrap()
                .units
                .get_mut(&id)
                .unwrap()
                .coasting = *enabled;
        }
        Action::Quiet { enabled } => {
            if e.kind != crate::types::PlatformKind::Sub {
                return Err("3.1 silent acceleration mode requires submarine".into());
            }
            w.rules
                .navigation
                .as_mut()
                .unwrap()
                .units
                .get_mut(&id)
                .unwrap()
                .quiet = *enabled;
        }
        Action::HeaveTo { enabled } => {
            if *enabled && astern::engaged(w, id) {
                return Err(
                    "3.1.3/3.1.5 heave-to and reverse steering combination requires adjudication"
                        .into(),
                );
            }
            if *enabled && w.context.flight_environment.is_none() {
                return Err("3.1.5 heave-to requires explicit wind direction".into());
            }
            w.rules
                .navigation
                .as_mut()
                .unwrap()
                .units
                .get_mut(&id)
                .unwrap()
                .heave_to = *enabled;
        }
    }
    trace(w, id, "3.order", json!({"action":action}));
    Ok(())
}
pub fn step(w: &mut World) {
    if !active(w) {
        return;
    }
    let ids: Vec<_> = w
        .rules
        .navigation
        .as_ref()
        .map(|s| s.units.keys().copied().collect())
        .unwrap_or_default();
    for id in ids {
        if w.rules.navigation.as_ref().unwrap().units[&id]
            .ready_at
            .is_some_and(|due| due <= w.tick)
        {
            let u = w
                .rules
                .navigation
                .as_mut()
                .unwrap()
                .units
                .get_mut(&id)
                .unwrap();
            u.ready_at = None;
            u.readiness = Readiness::Underway;
            trace(w, id, "3.6.ready", json!({"tick":w.tick}));
        }
    }
}
pub fn motion(w: &mut World, id: EntityId) -> Result<Option<Motion>, String> {
    if !configured(w, id) {
        return Ok(None);
    }
    astern::prepare(w, id)?;
    crashback::prepare(w,id)?;
    let e = w.entity(id).ok_or("navigation unit missing")?.clone();
    let now = w.tick;
    let u = w
        .rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap();
    let turn = tables()["turn_seconds"].as_i64().unwrap();
    if u.window != now / turn {
        u.window = now / turn;
        u.turn_degrees = 0.;
    }
    u.turn_degrees += crate::geom::angle_diff(e.course_deg, u.prior_course).abs();
    u.prior_course = e.course_deg;
    let fit = u.fit.clone();
    let u = u.clone();
    let rates = acceleration(&fit, e.speed_kts)?;
    let high = e.speed_kts
        > fit.undamaged_max_kts
            * tables()["acceleration"]["high_speed_fraction"]
                .as_f64()
                .unwrap();
    let turning = u.turn_degrees >= 45.
        || crate::geom::angle_diff(e.course_deg, e.ordered_course_deg).abs() >= 45.;
    let quiet_factor = tables()["acceleration"]["quiet_submarine_factor"]
        .as_f64()
        .unwrap();
    let quiet = if e.kind == crate::types::PlatformKind::Sub && u.quiet {
        quiet_factor
    } else {
        1.
    };
    let reverse = u.astern.as_ref().is_some_and(|s| s.moving_astern);
    let changing_direction = u
        .astern
        .as_ref()
        .is_some_and(|s| s.requested != s.moving_astern);
    let accel = rates[usize::from(high)]
        * if turning {
            tables()["acceleration"]["turn45_factor"].as_f64().unwrap()
        } else {
            1.
        }
        * quiet
        * if reverse {
            tables()["acceleration"]["astern_factor"].as_f64().unwrap()
        } else {
            1.
        }
        / turn as f64;
    let crash_back=crashback::active(w,id);
    let decel = rates[2]
        * if crash_back {tables()["crash_back"]["deceleration_factor"].as_f64().unwrap()}else{1.}
        * if u.coasting {
            tables()["acceleration"]["coasting_factor"]
                .as_f64()
                .unwrap()
        } else {
            1.
        }
        * quiet
        / turn as f64;
    let sea = w
        .context
        .radar_environment
        .as_ref()
        .ok_or("3.1.5 explicit sea state required for navigation fit")?
        .sea_state;
    let fraction = weather_fraction(
        fit.size,
        sea,
        e.kind == crate::types::PlatformKind::Sub,
        e.depth_m,
    )?;
    let mut maximum = e
        .max_speed_with_depth_rules(&w.db, fit.submarine.is_some())
        .min(fit.undamaged_max_kts);
    if let Some(limit) = super::battery2026::snorkel_speed_limit(w, id) {
        maximum = maximum.min(limit);
    }
    if let Some(surface) = submarine::surface_speed_limit(w, id) {
        maximum = maximum.min(surface);
    }
    let forward_maximum = maximum;
    if reverse {
        maximum = if e.kind == crate::types::PlatformKind::Sub {
            maximum.min(tables()["astern"]["submarine_max_kts"].as_f64().unwrap())
        } else {
            maximum
                * tables()["astern"]["surface_max_forward_fraction"]
                    .as_f64()
                    .unwrap()
        };
    }
    let mut minimum = 0.;
    let mut wind = None;
    match fraction {
        Some(f) => maximum = maximum.min(fit.undamaged_max_kts * f),
        None => {
            if !u.heave_to {
                return Err("3.1.5 H: must declare heave-to; no invented sinking timer".into());
            }
        }
    }
    if u.heave_to {
        let env = w
            .context
            .flight_environment
            .as_ref()
            .ok_or("3.1.5 wind direction missing")?;
        minimum = tables()["heave_kts"][0].as_f64().unwrap();
        maximum = maximum.min(tables()["heave_kts"][1].as_f64().unwrap());
        wind = Some(env.wind_from_deg);
        if maximum < minimum {
            return Err(
                "3.1.5 source ship capability below steerageway; adjudication required".into(),
            );
        }
    }
    maximum = maximum.min(match u.readiness {
        Readiness::ColdIron => 0.,
        Readiness::InportSteam => {
            fit.undamaged_max_kts
                * tables()["underway"]["inport_speed_fraction"]
                    .as_f64()
                    .unwrap()
        }
        _ => maximum,
    });
    maximum=maximum.min(super::battery2026::energy_movement_gate(w,id)?.unwrap_or(f64::INFINITY));
    if matches!(e.kind,crate::types::PlatformKind::Ship|crate::types::PlatformKind::Sub) && !super::expanded_damage2026::engineering_available(w,id)? {maximum=0.;}
    minimum = minimum.min(maximum);
    let target_kts = (changing_direction||crash_back).then_some(0.);
    let delta = target_kts
        .unwrap_or(e.ordered_speed_kts)
        .clamp(minimum, maximum)
        - e.speed_kts;
    let actual = delta.abs().min(if delta < 0. { decel } else { accel });
    let cavitating = e.kind == crate::types::PlatformKind::Sub
        && !u.quiet
        && actual
            > rates[if delta < 0. { 2 } else { usize::from(high) }] * quiet_factor / turn as f64
                + 1e-9;
    if let Some(e) = w.entity_mut(id) {
        if cavitating {
            e.ext.insert("navigation.cavitation".into(), 1.);
        } else {
            e.ext.remove("navigation.cavitation");
        }
    }
    let evidence = json!({"source":fit.source,"size":fit.size,"rates_per_turn":rates,"turn_seconds":turn,"high_speed_band":high,"turning45":turning,"coasting":u.coasting,"quiet":u.quiet,"cavitation":cavitating,"acceleration_per_s":accel,"deceleration_per_s":decel,"sea_state":sea,"sea_fraction":fraction,"forward_maximum_kts":forward_maximum,"maximum_kts":maximum,"moving_astern":reverse,"changing_direction":changing_direction,"heave_to":u.heave_to,"readiness":u.readiness});
    let signature = evidence.to_string();
    let unit = w
        .rules
        .navigation
        .as_mut()
        .unwrap()
        .units
        .get_mut(&id)
        .unwrap();
    if unit.last_motion.as_ref() != Some(&signature) {
        unit.last_motion = Some(signature);
        trace(w, id, "3.1.motion_limits", evidence);
    }
    let motion=Motion {
        minimum_kts: minimum,
        maximum_kts: maximum,
        acceleration_per_s: accel,
        deceleration_per_s: decel,
        wind_course: wind,
        target_kts,
    };
    hazards::constrain(w,id,motion).map(Some)
}
pub fn own_view(w: &World, id: EntityId) -> Option<Value> {
    let e=w.entity(id)?;
    let capability=(e.kind==crate::types::PlatformKind::Sub).then(||super::submarine_annex::capability(w,&e.class)).flatten();
    if !configured(w, id) {
        let mut view=match (capability,super::battery2026::own_view(w,id)) {
            (Some(cap),Some(battery))=>Some(json!({"annex_capability":cap,"battery":battery})),
            (Some(cap),None)=>Some(json!({"annex_capability":cap})),
            (None,Some(battery))=>Some(json!({"battery":battery})),
            (None,None)=>None,
        };
        if let Some(beached)=super::beached2026::own_view(w,id) {
            view.get_or_insert_with(||json!({}))["beached"]=beached;
        }
        return view;
    }
    // Whitelist projection: the raw unit carries private hard-rudder dice and
    // ledgers that must never reach a side observation.
    let unit = w.rules.navigation.as_ref()?.units.get(&id)?;
    let mut view=json!(unit);
    if let Some(map)=view.as_object_mut() {
        map.remove("hard_rudder");
        map.insert("steering".into(), hard::steering_view(&unit.hard_rudder, w.tick));
    }
    if let Some(cap)=capability {view["annex_capability"]=cap.clone();}
    if let Some(battery)=super::battery2026::own_view(w,id) {view["battery"]=battery;}
    if let Some(hazards)=hazards::own_view(w,id){view["hazards"]=hazards;}
    if let Some(beached)=super::beached2026::own_view(w,id){view["beached"]=beached;}
    Some(view)
}

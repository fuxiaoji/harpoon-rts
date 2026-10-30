//! Harpoon V2026 chapter 9; source round timing and discrete tables.
pub mod starting;
pub mod surface;
pub mod budget;
pub use surface::{begin_surface_defense_scan,end_surface_defense_scan,note_surface_defense,resolve_surface_attacks,SurfaceDefenseKind,defensive_source_unresolved,note_surface_defense_pending};
pub use starting::Configuration;
use crate::{EntityId, EventKind, Rng, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub surface: surface::State,
    #[serde(default)]
    pub attack_budget: budget::State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starting: Option<starting::State>,
    pub fights: BTreeMap<u64, Fight>,
    pub next_fight: u64,
    pub orders: BTreeMap<EntityId, Order>,
    pub positions: BTreeMap<EntityId, Position>,
    pub must_rtb: BTreeSet<EntityId>,
    pub diagnostics: BTreeSet<String>,
    pub evasions: BTreeMap<EntityId, BTreeSet<EntityId>>,
    pub targets: BTreeMap<EntityId, EntityId>,
    #[serde(default)]
    pub escaped_from: BTreeMap<EntityId, BTreeSet<EntityId>>,
    #[serde(default)]
    pub air_torpedo_last: BTreeMap<EntityId, i64>,
    #[serde(default)]
    pub interfered_torpedoes: BTreeSet<EntityId>,
    #[serde(default)]
    pub surface_prohibited_until: BTreeMap<EntityId, i64>,
    #[serde(default)]
    pub surface_attacks: BTreeMap<String, BTreeSet<EntityId>>,
    #[serde(default)]
    pub surface_last_turn: BTreeMap<EntityId, i64>,
    #[serde(default)]
    pub level_release_until: BTreeMap<EntityId, i64>,
    #[serde(default)]
    pub pending_surface_attacks: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub gun: i32,
    pub aam: i32,
    pub round: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fight {
    pub center: crate::Vec2,
    pub radius_nm: f64,
    pub altitude_m: f64,
    pub members: Vec<EntityId>,
    pub next_round: i64,
    pub rounds: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Order {
    Attack,
    Defend,
    Escape,
    NonManeuvering,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    StrafeAttack {contact:crate::ContactId,delivery:Delivery,aimpoint:String},
    ClusterAttack {
        contact:crate::ContactId,
        weapon:String,
        rounds:u16,
        delivery:Delivery,
        pattern:String,
        coverage:String,
    },
    CancelSurfaceAttack {},
    RocketAttack {
        contact: crate::ContactId,
        weapon: String,
        rounds: u16,
        aimpoint: String,
    },
    GuidedSurfaceAttack {
        weapon: String,
        rounds: u16,
        briefing: String,
    },
    Declare {
        contact: crate::ContactId,
    },
    Target {
        contact: crate::ContactId,
    },
    Attack,
    Defend,
    Escape,
    NonManeuvering,
    SurfaceAttack {
        contact: crate::ContactId,
        weapon: String,
        rounds: u16,
        delivery: Delivery,
        aimpoint: String,
    },
}
pub fn tables() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/aircombat-2026.json")).unwrap()
    })
}
pub fn in_dogfight(w: &World, id: EntityId) -> bool {
    w.rules
        .aircombat
        .as_ref()
        .is_some_and(|s| s.fights.values().any(|f| f.members.contains(&id)))
}
pub fn active(w: &World) -> bool {
    super::aviation2026::active(w)
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    starting::initialize(w)
}
fn profile(class: &str) -> Result<&'static Value, String> {
    tables()["profiles"]
        .get(class)
        .ok_or_else(|| format!("9.1/9.2 verified aircraft combat profile missing: {class}"))
}
pub fn gun_probability(gun: f64, damage: f64) -> Result<f64, String> {
    let col = tables()["gun_ratings"]
        .as_array()
        .unwrap()
        .iter()
        .position(|v| v.as_f64() == Some(gun))
        .ok_or(
            "9.4.2 gun rating is between printed columns; no verified interpolation instruction",
        )?;
    let row = tables()["gun_attack"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v[0].as_f64() == Some(damage))
        .ok_or(
            "9.4.2 damage rating is between printed rows; no verified interpolation instruction",
        )?;
    Ok(row[col + 1].as_f64().unwrap())
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Delivery {
    Glide,
    Level,
    Lob,
    Strafe,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Bombsight {
    Manual,
    Ballistic,
    Computing,
    Advanced,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnguidedInput {
    pub target_size: char,
    pub delivery: Delivery,
    pub altitude_m: f64,
    pub target_speed_kts: f64,
    pub bombsight: Bombsight,
    pub heavy_aa: bool,
    pub no_aa: bool,
    pub rudder: bool,
    pub land: bool,
    pub inertial_lob: bool,
}
pub fn unguided_probability(input: &UnguidedInput) -> Result<f64, String> {
    if !input.altitude_m.is_finite()
        || input.altitude_m < 0.
        || !input.target_speed_kts.is_finite()
        || input.target_speed_kts < 0.
        || input.heavy_aa && input.no_aa
    {
        return Err("9.6 invalid attack conditions".into());
    }
    let col = match input.delivery {
        Delivery::Glide => 0,
        Delivery::Level => 1,
        Delivery::Lob if input.inertial_lob => 0,
        Delivery::Lob => 2,
        Delivery::Strafe => 3,
    };
    let mut base = tables()["unguided_base"][input.target_size.to_string()][col]
        .as_f64()
        .ok_or("9.6 target size not printed")?;
    if input.delivery == Delivery::Level {
        base *= match input.altitude_m {
            h if h < 1500. => 3.,
            h if h < 3000. => 2.,
            h if h < 4000. => 1.,
            h if h < 5000. => 0.5,
            h if h <= 7500. => 0.25,
            _ => return Ok(0.),
        };
    }
    let mut shifts = if input.target_speed_kts <= 5. {
        2
    } else if input.target_speed_kts <= 10. {
        1
    } else if input.target_speed_kts < 25. {
        0
    } else if input.target_speed_kts < 35. {
        -1
    } else {
        -2
    };
    shifts += match input.bombsight {
        Bombsight::Manual => 0,
        Bombsight::Ballistic => 1,
        Bombsight::Computing => 3,
        Bombsight::Advanced => 5,
    };
    shifts += if input.rudder { 2 } else { 0 };
    shifts += if input.land { 2 } else { 0 };
    shifts += if input.no_aa {
        1
    } else if input.heavy_aa {
        -1
    } else {
        0
    };
    if input.delivery == Delivery::Glide && input.altitude_m > 2000. {
        shifts -= 2;
    }
    let rows = tables()["air_attack"].as_array().unwrap();
    let start = rows
        .iter()
        .position(|r| (r[0].as_f64().unwrap() - base).abs() < 1e-9)
        .ok_or("9.6 base chance between source table rows; no verified interpolation")?
        as i32;
    let finish = start - shifts;
    if finish < 0 || finish >= rows.len() as i32 {
        return Err("9.6 row shifts exceed printed Air Attack table; adjudication required".into());
    }
    Ok(rows[finish as usize][0].as_f64().unwrap())
}
pub fn air_attack_hits(probability: f64, salvo: u8, d100: u8) -> Result<u8, String> {
    if !(1..=4).contains(&salvo) || !(1..=100).contains(&d100) {
        return Err("9.6 one D100 applies to salvo size1–4".into());
    }
    if probability == 0. {
        return Ok(0);
    }
    let row = tables()["air_attack"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| (r[0].as_f64().unwrap() - probability).abs() < 1e-9)
        .ok_or("9.6 exact printed Air Attack row required")?;
    let start = [0, 1, 2, 4, 7][salvo as usize];
    Ok((1..=salvo)
        .rev()
        .find(|hit| d100 as f64 <= row[start + *hit as usize - 1].as_f64().unwrap() * 100. + 1e-9)
        .unwrap_or(0))
}
pub fn unguided_attack(input: &UnguidedInput, salvo: u8, d100: u8) -> Result<u8, String> {
    air_attack_hits(unguided_probability(input)?, salvo, d100)
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum PgmGuidance {
    CommandSarh,
    Tarh,
    Salh,
    Eoir,
    Satnav,
    Prh,
}
pub fn pgm_probability(
    guidance: PgmGuidance,
    generation: u8,
    target_vsmall: bool,
    ecm_generation: u8,
    los_lost: bool,
) -> Result<f64, String> {
    if ecm_generation > 4 {
        return Err("9.6.8 PGM countermeasures generation exceeds printed table".into());
    }
    let row = match guidance {
        PgmGuidance::CommandSarh | PgmGuidance::Prh if (1..=3).contains(&generation) => {
            generation as usize - 1
        }
        PgmGuidance::Tarh if generation == 4 => 3,
        PgmGuidance::Salh if (1..=3).contains(&generation) => generation as usize + 3,
        PgmGuidance::Eoir if (1..=3).contains(&generation) => generation as usize + 6,
        PgmGuidance::Satnav if (1..=3).contains(&generation) => generation as usize + 9,
        _ => return Err("9.6.8 seeker generation/type not printed".into()),
    };
    let small = target_vsmall && !matches!(guidance, PgmGuidance::Prh);
    Ok(
        tables()["pgm"][row][ecm_generation as usize * 2 + usize::from(small)]
            .as_f64()
            .unwrap()
            / if los_lost { 4. } else { 1. },
    )
}
pub fn rocket_hits(rockets: u16, dice: [u8; 2], target_size: char) -> Result<u16, String> {
    if dice.iter().any(|d| !(1..=6).contains(d)) {
        return Err("9.6.7 explicit2D6 required".into());
    }
    let modifier = match target_size {
        'A' => 2,
        'B' => 1,
        'C' | 'D' => 0,
        'E' => -1,
        'F' => -2,
        'G' => -3,
        'H' => -4,
        _ => return Err("9.6.7 size not printed".into()),
    };
    let total = dice[0] as i32 + dice[1] as i32 + modifier;
    if !(2..=12).contains(&total) {
        return Err(
            "9.6.7 modified2D6 outside printed columns; no unprinted result guessed".into(),
        );
    }
    let row = tables()["rocket_hits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r[0].as_u64() == Some(rockets as u64))
        .ok_or("9.6.7 rocket count not printed; split source-supported32-rocket groups")?;
    Ok(row[total as usize - 1].as_u64().unwrap() as u16)
}
pub fn satnav_release_allowed(
    generation: u8,
    altitude: f64,
    sight: Bombsight,
    moving: bool,
    terminal_seeker: bool,
) -> bool {
    matches!(sight, Bombsight::Computing | Bombsight::Advanced)
        && (!moving || terminal_seeker)
        && match generation {
            1 => altitude > 7500.,
            2 => altitude > 2000.,
            3 => altitude > 60.,
            _ => false,
        }
}
pub fn target_maneuver(w: &World, id: EntityId) -> Result<f64, String> {
    maneuver_base(w, id, false)
}
fn maneuver_base(w: &World, id: EntityId, surprise_round: bool) -> Result<f64, String> {
    let e = w.entity(id).ok_or("missing aircraft")?;
    if w.rules
        .aircombat
        .as_ref()
        .and_then(|s| s.level_release_until.get(&id))
        .is_some_and(|until| w.tick < *until)
    {
        return Ok(0.);
    }
    if !surprise_round && !maneuvering(w, id) {
        return Ok(0.);
    }
    let f = w
        .rules
        .aviation
        .as_ref()
        .and_then(|s| s.flights.get(&id))
        .ok_or("9.1 verified load mass required for Maneuver Rating")?;
    let a = super::aviation2026::tables()["maneuver"][&e.class]
        [usize::from(f.payload_fraction >= 0.5)]
    .as_f64()
    .ok_or("9.1 source light/heavy maneuver pair missing")?;
    let order = if surprise_round {Order::Attack} else {w
        .rules
        .aircombat
        .as_ref()
        .and_then(|s| s.orders.get(&id))
        .copied()
        .unwrap_or(Order::Attack)};
    let damaged =
        super::expanded_damage2026::aircraft_profile(w, id, a, f.payload_fraction >= 0.5)?;
    if e.altitude_m <= 60. {
        return Ok(damaged.maneuver_rating.min(0.5));
    }
    // Explicit RTS composition: damage changes the fitted/load MR first;
    // tactical defense then adds its source bonus. Fixed critical/structural
    // ratings remain fixed rather than acquiring a tactical bonus.
    let fixed = w
        .rules
        .expanded_damage
        .as_ref()
        .filter(|s| {
            s.config.optional.aircraft_damage != super::expanded_damage2026::AircraftMode::Off
        })
        .and_then(|s| s.units.get(&id))
        .and_then(|u| u.aircraft.as_ref())
        .is_some_and(|d| {
            !d.repaired
                && (matches!(
                    d.severity,
                    super::expanded_damage2026::AirSeverity::Critical
                        | super::expanded_damage2026::AirSeverity::Destroyed
                ) || d.severity == super::expanded_damage2026::AirSeverity::Light
                    && d.light.contains("structure"))
        });
    Ok(match order {
        Order::NonManeuvering => 0.,
        Order::Defend | Order::Escape if !fixed => damaged.maneuver_rating + 0.5,
        _ => damaged.maneuver_rating,
    })
}
pub fn target_signature(w: &World, id: EntityId, observer: EntityId) -> Result<usize, String> {
    let e = w.entity(id).ok_or("missing target")?;
    let o = w.entity(observer).ok_or("missing observer")?;
    let p = super::aviation2026::operations_aircraft(w, &e.class)
        .ok_or("9.5 verified aircraft source fit missing")?;
    let mut s = p["signature"]
        .as_u64()
        .ok_or("9.5 verified signature missing")? as usize;
    if p["front_half_deg"]
        .as_f64()
        .is_some_and(|v| crate::geom::angle_diff(e.course_deg, e.pos.bearing_to(o.pos)).abs() <= v)
    {
        s = p["front_signature"].as_u64().unwrap() as usize;
    }
    if p["external_ordnance"] == true {
        s = s.min(2);
    }
    Ok(s)
}
pub fn maneuvering(w: &World, id: EntityId) -> bool {
    w.rules
        .aircombat
        .as_ref()
        .and_then(|s| s.orders.get(&id))
        .copied()
        .unwrap_or(Order::Attack)
        != Order::NonManeuvering
        && w.entity(id).is_some_and(|e| {
            if active(w) { !starting::surprised(w,id) }
            else { e.ext.get("9.surprised") != Some(&1.0) }
        })
}
pub fn begin_aam_evasion(w: &mut World, target: EntityId, missile: EntityId) {
    if !active(w)
        || in_dogfight(w, target)
        || !w
            .entity(target)
            .is_some_and(|e| e.alive && e.kind == crate::types::PlatformKind::Aircraft)
    {
        return;
    }
    w.rules
        .aircombat
        .as_mut()
        .unwrap()
        .evasions
        .entry(target)
        .or_default()
        .insert(missile);
    super::aam2026::controller_evaded(w, target);
    trace(
        w,
        target,
        "9.5.1.engaged",
        json!({"maneuvering":maneuvering(w,target),"release_prohibited":true}),
    );
}
pub fn controlling_missiles_stupid(w: &World, actor: EntityId) -> bool {
    w.rules.aircombat.as_ref().is_some_and(|s| {
        s.evasions
            .get(&actor)
            .is_some_and(|m| m.iter().any(|id| w.entity(*id).is_some_and(|e| e.alive)))
    })
}
pub fn release_allowed(w: &World, actor: EntityId) -> Result<(), String> {
    if active(w) && w.entity(actor).is_some_and(|e|e.ext.get("nuclear.mission_abort").copied().unwrap_or(0.)>0.) {
        return Err("14.1.8 actual nuclear mission-abort prevents offensive weapon release".into());
    }
    if active(w) && starting::surprised(w,actor) {
        return Err("9.4.7 surprised aircraft cannot return fire during the one free round".into());
    }
    if active(w) && !super::expanded_damage2026::aircraft_profile(w, actor, 0., false)?.offensive {
        return Err("14.9 recorded aircraft damage prevents offensive weapon release".into());
    }
    if active(w) && controlling_missiles_stupid(w, actor) {
        Err(
            "9.5.1 aircraft engaged by AAM may only jettison; new ordnance release prohibited"
                .into(),
        )
    } else {
        Ok(())
    }
}
/// Called by the actual AAM preparation adapter with its selected weapon.
/// A passive/IR surprise declaration cannot silently launch a radar weapon.
pub fn starting_weapon_gate(w: &World, actor: EntityId, weapon: &str) -> Result<(), String> {
    starting::weapon_gate(w,actor,weapon)
}
/// §9.6 prohibits a naval/ground attack in any Tactical Turn that includes
/// dogfighting, including the remainder of the turn after a fight ends.
pub fn surface_attack_gate(w: &World, actor: EntityId) -> Result<(), String> {
    if !active(w)
        || !w
            .entity(actor)
            .is_some_and(|e| e.kind == crate::types::PlatformKind::Aircraft)
    {
        return Ok(());
    }
    release_allowed(w, actor)?;
    if in_dogfight(w, actor)
        || w.rules
            .aircombat
            .as_ref()
            .and_then(|s| s.surface_prohibited_until.get(&actor))
            .is_some_and(|until| w.tick < *until)
    {
        return Err("9.6 aircraft involved in a dogfight may not attack naval/ground targets in that Tactical Turn".into());
    }
    Ok(())
}
pub fn air_torpedo_gate(
    w: &World,
    actor: EntityId,
    contact: crate::ContactId,
) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    release_allowed(w, actor)?;
    let a = w
        .entity(actor)
        .filter(|a| a.alive)
        .ok_or("9.7.1 aircraft missing")?;
    if a.kind != crate::types::PlatformKind::Aircraft {
        return Ok(());
    }
    if !w
        .rules
        .aviation
        .as_ref()
        .is_some_and(|state| state.flights.contains_key(&actor))
    {
        return Err("9.7.1 explicit verified aircraft load/fuel state required before aerial torpedo release".into());
    }
    let c = w.sides[a.side.index()]
        .contacts
        .get(&contact)
        .ok_or("9.7.1 own or shared target location required")?;
    if c.stale || c.bearing_only || !c.estimate.x.is_finite() || !c.estimate.y.is_finite() {
        return Err(
            "9.7.1 a current located contact is required, not stale/bearing-only data".into(),
        );
    }
    if super::detection2026::wide_mad::only_mad_waiting(w,actor,c.about) {
        return Err("5.7.1 a MAD-only fix permits attack in the next Tactical Turn".into());
    }
    if super::detection2026::known_category(w, a.side, c.about)
        != Some(crate::types::PlatformKind::Sub)
    {
        return Err("9.7.1 a source-classified submarine contact is required".into());
    }
    let s = super::aviation2026::spec(&a.class)?;
    let rotary = s.engine == super::aviation2026::Engine::TS;
    let d = &tables()["airborne_asw"];
    let low_min = d["low_min_exclusive_m"].as_f64().unwrap();
    let low_max = d["low_max_m"].as_f64().unwrap();
    if !rotary && !(low_min < a.altitude_m && a.altitude_m <= low_max)
        || rotary && !(0.0..=low_max).contains(&a.altitude_m)
    {
        return Err(
            "9.7.1 torpedoes require Low altitude; helicopters may also drop in hover".into(),
        );
    }
    let fmp = super::aviation2026::source_speed(
        &a.class,
        a.altitude_m,
        super::aviation2026::Throttle::Fmp,
    )?;
    if !a.speed_kts.is_finite()
        || a.speed_kts < 0.
        || a.speed_kts > fmp * d["maximum_fmp_fraction"].as_f64().unwrap()
    {
        return Err("9.7.1 torpedo drop exceeds 50% source FMP speed".into());
    }
    if w.rules
        .aircombat
        .as_ref()
        .and_then(|s| s.air_torpedo_last.get(&actor))
        .is_some_and(|last| w.tick - *last < d["turn_seconds"].as_i64().unwrap())
    {
        return Err("9.7.1 one aerial torpedo per Tactical Turn (180s rolling RTS window)".into());
    }
    Ok(())
}
/// Register a real water entry and its physical interference before terminal
/// processing. The source Flight retains the entry datum; torpedo2026 consumes
/// its6.3.8 placement/10.2.5 attack after all manual and automatic drops for the
/// tick have registered. Never resolve an attack in this per-launch callback.
pub fn air_torpedo_dropped(w: &mut World, actor: EntityId, torpedo: EntityId) {
    if !active(w)
        || !w
            .entity(actor)
            .is_some_and(|e| e.kind == crate::types::PlatformKind::Aircraft)
    {
        return;
    }
    let Some(p) = w
        .entity(torpedo)
        .filter(|e| e.alive && e.kind == crate::types::PlatformKind::Torpedo)
        .and_then(|e| e.projectile.as_ref())
        .filter(|p| p.launched_by == actor)
    else {
        return;
    };
    let target = p.target;
    w.rules
        .aircombat
        .as_mut()
        .unwrap()
        .air_torpedo_last
        .insert(actor, w.tick);
    trace(
        w,
        actor,
        "9.7.1.torpedo_entered_water",
        json!({"torpedo":torpedo,"next_drop_at":w.tick+tables()["airborne_asw"]["turn_seconds"].as_i64().unwrap(),"source_pdf":109}),
    );
    let Some(target) = target else {
        return;
    };
    let others: Vec<_> = w
        .entities
        .values()
        .filter(|e| {
            e.alive
                && e.id != torpedo
                && e.kind == crate::types::PlatformKind::Torpedo
                && e.projectile.as_ref().is_some_and(|p| {
                    p.target == Some(target)
                        && w.entity(p.launched_by)
                            .is_some_and(|a| a.kind == crate::types::PlatformKind::Aircraft)
                })
        })
        .map(|e| e.id)
        .collect();
    if !others.is_empty() {
        let state = w.rules.aircombat.as_mut().unwrap();
        state.interfered_torpedoes.insert(torpedo);
        state.interfered_torpedoes.extend(others.iter().copied());
        let side = w.entity(actor).map(|e| e.side);
        w.emit(
            EventKind::RulesEvent {
                domain: "operations".into(),
                rule: "9.7.1.torpedo_interference".into(),
                side,
                unit: Some(actor),
                contact: None,
                detail: json!({"both_miss":true,"torpedo":torpedo,"other_torpedoes":others,
                    "other_count":others.len(),"source":"9.7.1 / 7.5.3"})
                .to_string(),
            },
            Visibility::Referee,
        );
    }
}
pub fn air_torpedo_interfered(w: &World, id: EntityId) -> bool {
    active(w)
        && w.rules
            .aircombat
            .as_ref()
            .is_some_and(|s| s.interfered_torpedoes.contains(&id))
}
pub fn own_view(w: &World, id: EntityId) -> Option<Value> {
    if !active(w) {
        return None;
    }
    let s = w.rules.aircombat.as_ref()?;
    let aerial_torpedo_release = w
        .entity(id)
        .filter(|e| e.kind == crate::types::PlatformKind::Aircraft)
        .map(|e| {
            let d = &tables()["airborne_asw"];
            let ready_at = s
                .air_torpedo_last
                .get(&id)
                .map(|last| last + d["turn_seconds"].as_i64().unwrap())
                .unwrap_or(w.tick);
            let rotary = super::aviation2026::spec(&e.class)
                .is_ok_and(|p| p.engine == super::aviation2026::Engine::TS);
            let maximum_speed = super::aviation2026::source_speed(
                &e.class,
                e.altitude_m,
                super::aviation2026::Throttle::Fmp,
            )
            .ok()
            .map(|fmp| fmp * d["maximum_fmp_fraction"].as_f64().unwrap());
            json!({"ready_at":ready_at,"remaining_s":(ready_at-w.tick).max(0),
                "max_altitude_m":d["low_max_m"],"rotary_allow_hover":rotary,
                "max_speed_kts":maximum_speed})
        });
    let fight=s.fights.iter().find(|(_,f)|f.members.contains(&id)).map(|(key,f)|json!({"id":key,"center":f.center,"radius_nm":f.radius_nm,"rounds":f.rounds,"next_round":f.next_round,"order":s.orders.get(&id),"position":s.positions.get(&id)}));
    Some(
        json!({"dogfight":fight,"must_return":s.must_rtb.contains(&id),"engaged_by_aam":controlling_missiles_stupid(w,id),"maneuvering":maneuvering(w,id),"starting_conditions":starting::own_view(w,id),"naval_ground_attack_prohibited_until":s.surface_prohibited_until.get(&id),"last_bombing_turn":s.surface_last_turn.get(&id),"level_nonmaneuvering_until":s.level_release_until.get(&id),"next_air_torpedo_drop_at":s.air_torpedo_last.get(&id).map(|last|last+tables()["airborne_asw"]["turn_seconds"].as_i64().unwrap()),"aerial_torpedo_release":aerial_torpedo_release,"surface_pass":surface::own_view(w,id),"guided_air_ordnance":super::pgm2026::own_view(w,id),"rockets":super::rocket2026::own_view(w,id),"cluster_ordnance":super::cluster2026::own_view(w,id),"strafing":super::strafe2026::own_view(w,id)}),
    )
}
pub fn aam_position_allowed(w: &World, id: EntityId, aspect: &str) -> bool {
    if !in_dogfight(w, id) {
        return true;
    }
    if w.rules
        .aircombat
        .as_ref()
        .and_then(|s| s.orders.get(&id))
        .is_some_and(|o| *o != Order::Attack)
    {
        return false;
    }
    let Some(p) = w
        .rules
        .aircombat
        .as_ref()
        .and_then(|s| s.positions.get(&id))
    else {
        return false;
    };
    let threshold = match aspect {
        "all" => 51,
        "wide" => 61,
        "narrow" => 71,
        _ => return false,
    };
    p.aam >= threshold
}
fn trace(w: &mut World, id: EntityId, rule: &str, detail: Value) {
    super::aviation2026::trace(w, id, rule, detail)
}
fn blocked(w: &mut World, id: EntityId, rule: &str, reason: &str) {
    let key = format!("{}:{rule}:{reason}", id.0);
    if w.rules
        .aircombat
        .as_mut()
        .is_some_and(|s| s.diagnostics.insert(key))
    {
        trace(w, id, rule, json!({"status":"blocked","reason":reason}));
    }
}
pub fn altitude_band(m: f64) -> usize {
    if m <= 20. {
        0
    } else if m <= 60. {
        1
    } else if m <= 2000. {
        2
    } else if m <= 7500. {
        3
    } else if m <= 15500. {
        4
    } else if m <= 30000. {
        5
    } else {
        6
    }
}
pub fn radius_for_fastest(speed: f64) -> Result<f64, String> {
    if !speed.is_finite() || speed < 0. {
        return Err("9.4 invalid speed".into());
    }
    tables()["dogfight_radius"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| speed <= r[0].as_f64().unwrap())
        .map(|r| r[1].as_f64().unwrap())
        .ok_or("9.4 speed exceeds source radius envelope".into())
}
fn max_speed(class: &str) -> Result<f64, String> {
    super::aviation2026::tables()["speed_rows"][class]
        .as_array()
        .map(|rows| {
            rows.iter()
                .flat_map(|r| r.as_array().unwrap())
                .filter_map(|v| v.as_f64())
                .fold(0., f64::max)
        })
        .ok_or("9.4 source maximum aircraft speed missing".into())
}
fn cloud_gate(w: &World) -> Result<(), String> {
    if w.weather.overcast > 0. {
        Err(
            "9.4.8 cloud presence without verified local clear flight volume; dogfight blocked"
                .into(),
        )
    } else {
        Ok(())
    }
}
/// Published Mk82 delivery subset. No inferred bomb aliases, AA modifier,
/// aircraft sight, target size, or impact penetration enters this path.
pub fn surface_attack(w:&mut World,actor:EntityId,contact:crate::ContactId,weapon:&str,rounds:u16,delivery:Delivery,aimpoint:&str)->Result<(),String> {
    surface::declare(w,actor,contact,weapon,rounds,delivery,aimpoint)
}
pub(super) fn execute_surface_release(w:&mut World,p:&surface::PreparedRelease,no_aa:bool,heavy_aa:bool)->Result<(),String> {
    use crate::types::PlatformKind;
    let actor=p.actor;let contact=p.contact;let weapon=p.weapon.as_str();let rounds=p.rounds;let delivery=p.delivery;let aimpoint=p.aimpoint.as_str();
    let a=w.entity(actor).ok_or("9.6 source aircraft unavailable at release")?.clone();
    let target=p.target;let known_kind=p.known_kind;let no_aa_source=p.no_aa_source.clone();
    let aim=super::aviation2026::SurfaceAimpoint{source:p.aim_source.clone(),point:p.point,controlled_range:p.native.is_none(),fixed_target:p.native.is_none(),allowed_sides:vec![p.side]};
    let s=&tables()["surface_attack"];let turn=w.tick.div_euclid(s["turn_seconds"].as_i64().unwrap());let key=format!("{}:{turn}",target.0);
    let slot=a.weapons.iter().position(|m|m.weapon==weapon&&m.mounts>0&&m.ammo>=rounds).ok_or("9.6 release stock no longer available")?;
    let def=w.db.weapons.get(weapon).cloned().ok_or("9.6 source bomb definition absent")?;let data=&tables()["surface_bombs"][weapon];
    let altitude=p.anchor.altitude_m;
    let sight=surface::bombsight(&a.class)?;
    let high_level=delivery==Delivery::Level&&super::operations::altitude_band(altitude)==s["high_level_band"].as_u64().unwrap() as usize;
    let high_free=high_level&&s["high_level_after_release_maneuver_free"].as_bool().unwrap();
    let pen=if high_level{None}else{Some(data[if delivery==Delivery::Glide{"glide_penetration_cm"}else{"level_penetration_cm"}][usize::from(altitude>2000.)].as_f64().ok_or("9.6 source penetration cell missing")?)};
    let mut impact = def.clone();
    impact.rule_data = Some(super::damage_runtime::WeaponDamage {
        ship_dp: data["damage_points"].as_f64(),
        penetration_cm: pen,
        profile: if delivery == Delivery::Glide {
            "dive"
        } else {
            "level"
        }
        .into(),
        bomb_kind: "LDGP".into(),
        source: data["source"].as_str().unwrap().into(),
        ..Default::default()
    });

    // All action admission above uses own information and explicit permissions.
    // Commit the physical release before any hidden target-property query.
    w.entity_mut(actor).unwrap().weapons[slot].ammo -= rounds;
    super::aviation2026::ordnance_released(w, actor, weapon, rounds);
    let state = w.rules.aircombat.as_mut().unwrap();
    state.surface_attacks.entry(key).or_default().insert(actor);
    state.surface_last_turn.insert(actor, turn);
    let level_nonmaneuvering_until=(delivery==Delivery::Level&&!high_free).then(||w.tick+s["drop_increment_s"].as_i64().unwrap());
    if let Some(until)=level_nonmaneuvering_until {
        state
            .level_release_until
            .insert(actor,until);
    } else if high_free {
        state.level_release_until.remove(&actor);
    }
    w.emit(
        EventKind::WeaponsRelease {
            side: a.side,
            shooter: actor,
            shooter_name: a.name.clone(),
            weapon: def.name,
            count: rounds as u8,
        },
        Visibility::Side(a.side),
    );
    trace(
        w,
        actor,
        "9.6.bombs_released",
        json!({"contact":contact,"weapon":weapon,"delivery":delivery,"rounds":rounds,"aimpoint":aimpoint,"level_maneuver_zero_until":level_nonmaneuvering_until,"high_level_after_release_free":high_free}),
    );

    let mut adjudication = json!({"contact":contact,"target":target,"weapon":weapon,"delivery":delivery,"rounds":rounds,"impact_penetration_cm":pen,"no_aa_source":no_aa_source,"aimpoint":aimpoint,"aimpoint_source":aim.source,"declared_point":aim.point,"native_measurement":p.native,"defense":{"no_aa":no_aa,"heavy_aa":heavy_aa},"logical_release_anchor":p.anchor});
    let ready = (|| -> Result<(crate::world::Entity, char, f64, bool), (&'static str, String)> {
        let t = w.entity(target).cloned().ok_or((
            "pending_source_target_properties",
            "target entity unavailable to referee".into(),
        ))?;
        adjudication["target_position"] = json!(t.pos);
        if !t.alive || t.kind != known_kind {
            return Err(("pending_source_target_properties","referee target state/category differs from own contact; no source assessment bound".into()));
        }
        let mut source_runway = false;
        let (size, armor) = if t.kind == PlatformKind::Base {
            let ground = super::ground2026::source_target(w, target)
                .map_err(|e| ("pending_source_target_properties", e))?;
            let runway = super::ground2026::template("runway")
                .map_err(|e| ("pending_source_target_properties", e))?;
            source_runway = ground.effect == runway.effect
                && ground.size == runway.size
                && ground.damage_points == runway.damage_points
                && ground.vertical_armor == runway.vertical_armor
                && ground.horizontal_armor == runway.horizontal_armor;
            (ground.size, ground.horizontal_armor)
        } else {
            let p = t
                .rule_data
                .as_ref()
                .or_else(|| super::damage_runtime::platform(&t.class))
                .ok_or((
                    "pending_source_target_properties",
                    "source target physical size missing".into(),
                ))?;
            (
                p.size.chars().next().ok_or((
                    "pending_source_target_properties",
                    "source target physical size missing".into(),
                ))?,
                p.deck_cm,
            )
        };
        adjudication["target_size"] = json!(size);
        adjudication["target_horizontal_armor_cm"] = json!(armor);
        if !"ABCDEFGH".contains(size) || !armor.is_finite() || armor < 0. {
            return Err((
                "pending_source_target_properties",
                "source target size/armor unresolved".into(),
            ));
        }
        if t.kind == PlatformKind::Base && armor > 0. && !source_runway {
            return Err((
                "pending_source_target_properties",
                "protected facility critical-hit mapping not source bound".into(),
            ));
        }
        if source_runway {
            if !pen.is_some_and(|p| p > armor) {
                return Err(("pending_source_target_properties",
                    "11.5.1 concrete runway requires source-qualified bomb penetration; nonpenetrating outcome remains pending".into()));
            }
            let maximum = super::ground2026::runway_salvo(
                rounds, delivery == Delivery::Level, true, altitude, false,
            ).map_err(|e| ("pending_source_target_properties", e))?;
            adjudication["runway"] = json!({
                "source":super::ground2026::tables()["runway_bomb_attack"]["source"],
                "maximum_salvo":maximum,
                "level_selection":super::ground2026::tables()["runway_bomb_attack"]["current_level_selection"],
                "max_hits_per_salvo":super::ground2026::tables()["runway_bomb_attack"]["max_hits_per_salvo"]
            });
        }
        if !t.pos.x.is_finite()
            || !t.pos.y.is_finite()
            || t.pos.range_to(aim.point) > s["tolerance_nm"].as_f64().unwrap()
            || t.speed_kts != 0.
        {
            return Err((
                "pending_source_placement",
                "actual target placement/movement outside the captured source aim; no guessed dispersion or target teleport".into(),
            ));
        }
        Ok((t, size, armor, source_runway))
    })();
    match ready {
        Err((resolution, reason)) => {
            adjudication["resolution"] = json!(resolution);
            adjudication["pending_reason"] = json!(reason);
            w.rules
                .aircombat
                .as_mut()
                .unwrap()
                .pending_surface_attacks
                .insert(format!("{}:{turn}", actor.0), adjudication.clone());
        }
        Ok((t, size, _armor, source_runway)) => {
            let input = UnguidedInput {
                target_size: size,
                delivery,
                altitude_m: altitude,
                target_speed_kts: t.speed_kts,
                bombsight: sight,
                heavy_aa,
                no_aa,
                rudder: t.ext.get("damage.rudder_jammed").is_some_and(|v| *v > 0.),
                land: t.kind == PlatformKind::Base,
                inertial_lob: false,
            };
            match unguided_probability(&input) {
                Err(reason) => {
                    adjudication["resolution"] = json!("pending_source_target_properties");
                    adjudication["pending_reason"] = json!(reason);
                    w.rules
                        .aircombat
                        .as_mut()
                        .unwrap()
                        .pending_surface_attacks
                        .insert(format!("{}:{turn}", actor.0), adjudication.clone());
                }
                Ok(probability) => {
                    let mut salvos = vec![];
                    let mut left = rounds;
                    if delivery == Delivery::Level {
                        salvos.push(1);
                    } else {
                        while left > 0 {
                            let count = left.min(4);
                            salvos.push(count as u8);
                            left -= count;
                        }
                    }
                    let mut results = vec![];
                    let mut total_hits = 0u16;
                    for (group, count) in salvos.into_iter().enumerate() {
                        let d100 =
                            roll(w, actor, &format!("9.6.Mk82:{weapon}:{turn}:{group}"), 100);
                        // probability and printed group count were validated above.
                        let hits = air_attack_hits(probability, count, d100)
                            .expect("verified source probability/group");
                        let hits = if source_runway {
                            hits.min(super::ground2026::tables()["runway_bomb_attack"]["max_hits_per_salvo"].as_u64().unwrap() as u8)
                        } else { hits };
                        total_hits += hits as u16;
                        results.push(json!({"salvo":count,"d100":d100,"hits":hits}));
                    }
                    adjudication["resolution"] = json!(if p.native.is_some(){"source_native_visual"}else{"source_fixed_range"});
                    adjudication["probability"] = json!(probability);
                    adjudication["groups"] = json!(results);
                    adjudication["total_hits"] = json!(total_hits);
                    for n in 0..total_hits {
                        if t.kind == PlatformKind::Base {
                            let id = w.allocate_id();
                            w.rules.aaw.as_mut().unwrap().surface.land_hits.insert(
                                id,
                                super::surface2026::LandHit {
                                    target,
                                    weapon: impact.clone(),
                                    shooter_side: a.side,
                                    damage: data["damage_points"].as_f64().unwrap(),
                                },
                            );
                        } else {
                            super::damage_runtime::queue(
                                w,
                                target,
                                &impact,
                                false,
                                Some(0.),
                                Some(a.side),
                                Some(format!("9.6:{}:{turn}:{n}", actor.0)),
                            );
                        }
                    }
                }
            }
        }
    }
    w.emit(
        EventKind::RulesEvent {
            domain: "aircombat2026".into(),
            rule: "9.6.source_bomb_attack".into(),
            side: Some(a.side),
            unit: Some(actor),
            contact: Some(contact),
            detail: adjudication.to_string(),
        },
        Visibility::Referee,
    );
    Ok(())
}
pub fn configure_database(db: &mut crate::Database) {
    use crate::db::{Guidance, Targets, WeaponDef, WeaponFit, WeaponKind};
    for (id, p) in tables()["surface_bombs"].as_object().unwrap() {
        db.weapons.insert(
            id.clone(),
            WeaponDef {
                id: id.clone(),
                name: p["name"].as_str().unwrap().into(),
                kind: WeaponKind::Bomb,
                targets: Targets {
                    surface: true,
                    land: true,
                    ..Default::default()
                },
                range_nm: 6.,
                min_range_nm: 0.,
                speed_kts: 0.,
                enable_range_nm: 0.,
                pk: 0.,
                salvo: 1,
                reload_s: 0.,
                damage: 0.,
                guidance: Guidance::BallisticGun,
                ir: false,
                directors: 0,
                rule_data: None,
            },
        );
    }
    for (alias, fit) in tables()["surface_fits"].as_object().unwrap() {
        if let Some(mut plane) = db.platforms.get(fit["base"].as_str().unwrap()).cloned() {
            plane.id = alias.clone();
            plane.name.push_str(" Mk82 source load");
            plane.weapons.retain(|m| {
                db.weapons
                    .get(&m.weapon)
                    .is_some_and(|d| matches!(d.kind, WeaponKind::Aam))
            });
            plane.weapons.push(WeaponFit {
                weapon: fit["weapon"].as_str().unwrap().into(),
                mounts: fit["rounds"].as_u64().unwrap() as u8,
                ammo: 1,
            });
            db.platforms.insert(alias.clone(), plane);
        }
    }
    // Exact source fit only; this does not reinterpret the generic two-round
    // MH60R inventory or infer a torpedo's host-specific external mass.
    for (alias, fit) in super::aviation2026::tables()["external_store_fits"]
        .as_object()
        .unwrap()
    {
        if let Some(mut plane) = db.platforms.get(fit["base"].as_str().unwrap()).cloned() {
            plane.id = alias.clone();
            plane.name = fit["name"].as_str().unwrap().into();
            plane.weapons = vec![WeaponFit {
                weapon: fit["weapon"].as_str().unwrap().into(),
                mounts: fit["rounds"].as_u64().unwrap() as u8,
                ammo: 1,
            }];
            db.platforms.insert(alias.clone(), plane);
        }
    }
}
pub fn apply(w: &mut World, unit: EntityId, action: &Action) -> Result<(), String> {
    if !active(w) {
        return Err("9 source combat requires explicit hv2026-rts-v4".into());
    }
    let e = w
        .entity(unit)
        .filter(|e| e.alive && e.kind == crate::types::PlatformKind::Aircraft)
        .ok_or("9 aircraft unavailable")?
        .clone();
    match action {
        Action::StrafeAttack{contact,delivery,aimpoint}=>{
            return super::strafe2026::declare(w,unit,*contact,*delivery,aimpoint);
        }
        Action::ClusterAttack {contact,weapon,rounds,delivery,pattern,coverage} => {
            return super::cluster2026::declare(w,unit,*contact,weapon,*rounds,*delivery,pattern,coverage);
        }
        Action::RocketAttack {contact,weapon,rounds,aimpoint} => {
            return super::rocket2026::declare(w,unit,*contact,weapon,*rounds,aimpoint);
        }
        Action::GuidedSurfaceAttack { weapon, rounds, briefing } => {
            return super::pgm2026::declare(w, unit, weapon, *rounds, briefing);
        }
        Action::CancelSurfaceAttack {} => {
            surface::cancel(w, unit);
            return Ok(());
        }
        Action::SurfaceAttack {
            contact,
            weapon,
            rounds,
            delivery,
            aimpoint,
        } => {
            if *delivery==Delivery::Strafe {
                return super::strafe2026::declare_legacy(w,unit,*contact,weapon,*rounds,*delivery,aimpoint);
            }
            return surface_attack(w, unit, *contact, weapon, *rounds, *delivery, aimpoint);
        }
        Action::Target { contact } => {
            let c = w.sides[e.side.index()]
                .contacts
                .get(contact)
                .ok_or("9.4.1 own target contact required")?;
            let target = c.about;
            if !w
                .rules
                .aircombat
                .as_ref()
                .unwrap()
                .fights
                .values()
                .any(|f| f.members.contains(&unit) && f.members.contains(&target))
                || !w
                    .entity(target)
                    .is_some_and(|e| e.alive && e.side != w.entity(unit).unwrap().side)
            {
                return Err("9.4.1 target must be a live opponent in this dogfight".into());
            }
            w.rules
                .aircombat
                .as_mut()
                .unwrap()
                .targets
                .insert(unit, target);
            trace(w, unit, "9.4.1.target", json!({"contact":contact}));
        }
        Action::Declare { contact } => {
            if in_dogfight(w, unit) {
                return Err("9.4 aircraft already in dogfight".into());
            }
            if w.rules.aircombat.as_ref().unwrap().must_rtb.contains(&unit) {
                return Err("9.4.6 departing aircraft must return to base and may not declare another dogfight".into());
            }
            cloud_gate(w)?;
            profile(&e.class)?;
            target_maneuver(w, unit)?;
            let c = w.sides[e.side.index()]
                .contacts
                .get(contact)
                .ok_or("9.4 own target contact required")?;
            let t = w
                .entity(c.about)
                .filter(|t| {
                    t.alive && t.side != e.side && t.kind == crate::types::PlatformKind::Aircraft
                })
                .ok_or("9.4 live enemy aircraft contact required")?
                .clone();
            if w.rules
                .aircombat
                .as_ref()
                .unwrap()
                .escaped_from
                .get(&t.id)
                .is_some_and(|members| members.contains(&unit))
            {
                return Err("9.4.5 successfully escaped opponent cannot be pursued".into());
            }
            profile(&t.class)?;
            target_maneuver(w, t.id)?;
            let (a, b) = (altitude_band(e.altitude_m), altitude_band(t.altitude_m));
            if a <= 1 || b <= 1 || a + 1 < b || a > b + 2 {
                return Err("9.4 target outside source declaration altitude bands; VLow/SSkim cannot dogfight".into());
            }
            let members=starting::members(w,unit,t.id)?;
            let mut fastest=0.0f64;
            for id in &members {fastest=fastest.max(max_speed(&w.entity(*id).unwrap().class)?);}
            let radius = radius_for_fastest(fastest)?;
            if e.pos.range_to(t.pos) > radius {
                return Err("9.4 target beyond source dogfight radius".into());
            }
            let existing = w
                .rules
                .aircombat
                .as_ref()
                .unwrap()
                .fights
                .iter()
                .find(|(_, f)| f.members.contains(&t.id))
                .map(|(k, _)| *k);
            if existing.is_none() {
                let key=w.rules.aircombat.as_ref().unwrap().next_fight+1;
                starting::begin(w,key,unit,&members,w.tick+60)?;
            }
            let altitude=members.iter().map(|id|w.entity(*id).unwrap().altitude_m).fold(f64::INFINITY,f64::min);
            let s = w.rules.aircombat.as_mut().unwrap();
            let turn = tables()["airborne_asw"]["turn_seconds"].as_i64().unwrap();
            let until = (w.tick.div_euclid(turn) + 1) * turn;
            for id in &members {s.surface_prohibited_until.insert(*id, until);}
            let fight = if let Some(key) = existing {
                for id in &members {
                    if !s.fights[&key].members.contains(id) {s.fights.get_mut(&key).unwrap().members.push(*id);}
                }
                key
            } else {
                s.next_fight += 1;
                let key = s.next_fight;
                s.fights.insert(
                    key,
                    Fight {
                        center: e.pos,
                        radius_nm: radius,
                        altitude_m: altitude,
                        members: members.clone(),
                        next_round: w.tick + 60,
                        rounds: 0,
                    },
                );
                key
            };
            s.orders.insert(unit, Order::Attack);
            for id in members {s.orders.entry(id).or_insert(Order::Attack);}
            trace(
                w,
                unit,
                "9.4.declared",
                json!({"contact":contact,"fight":fight,"round_seconds":60,"radius_nm":radius}),
            );
        }
        order => {
            if !in_dogfight(w, unit) && !matches!(order, Action::NonManeuvering | Action::Attack) {
                return Err("9.4 aircraft is not in a dogfight".into());
            }
            let order = match order {
                Action::Attack => Order::Attack,
                Action::Defend => Order::Defend,
                Action::Escape => Order::Escape,
                Action::NonManeuvering => Order::NonManeuvering,
                _ => unreachable!(),
            };
            w.rules
                .aircombat
                .as_mut()
                .unwrap()
                .orders
                .insert(unit, order);
            trace(w, unit, "9.4.order", json!({"order":order}));
        }
    }
    Ok(())
}
pub fn position_result(
    d100: u8,
    attacker_mr: f64,
    defender_mr: f64,
    attacker_fmp: f64,
    defender_fmp: f64,
    helmet: u8,
) -> Result<Position, String> {
    if !(1..=100).contains(&d100)
        || helmet > 3
        || ![attacker_mr, defender_mr, attacker_fmp, defender_fmp]
            .iter()
            .all(|v| v.is_finite() && *v >= 0.)
    {
        return Err("9.4.1 invalid position inputs".into());
    }
    let p = &tables()["position"];
    let delta = p["fmp_delta_kts"].as_f64().unwrap();
    let m = p["fmp_modifier_percent"].as_i64().unwrap() as i32;
    let speed = if attacker_fmp - defender_fmp >= delta {
        m
    } else if defender_fmp - attacker_fmp >= delta {
        -m
    } else {
        0
    };
    let gun = d100 as i32
        + ((attacker_mr - defender_mr) * p["maneuver_diff_percent"].as_f64().unwrap()).round()
            as i32
        + speed;
    Ok(Position {
        gun,
        aam: gun + p["helmet_percent"][helmet as usize].as_i64().unwrap() as i32,
        round: 0,
    })
}
fn roll(w: &World, id: EntityId, purpose: &str, sides: u8) -> u8 {
    let mut r = w.rng.derive(Rng::hash_tag(w.tick, id.0, purpose));
    (r.next_f64() * sides as f64).floor() as u8 + 1
}
fn return_to_base(w: &mut World, id: EntityId) {
    if let Some(e) = w.entity_mut(id) {
        e.mission = Some(crate::world::MissionState::Rtb);
        e.assigned = None;
    }
    w.rules.aircombat.as_mut().unwrap().must_rtb.insert(id);
    w.rules.aircombat.as_mut().unwrap().positions.remove(&id);
    w.rules.aircombat.as_mut().unwrap().orders.remove(&id);
    w.rules.aircombat.as_mut().unwrap().targets.remove(&id);
}
fn departing_altitude(
    w: &mut World,
    id: EntityId,
    start: f64,
    rounds: u8,
    purpose: &str,
) -> (i32, usize) {
    let d = roll(w, id, purpose, 10) as i32 - rounds as i32;
    let drop = if d <= 1 {
        2
    } else if d <= 5 {
        1
    } else {
        0
    };
    let band = altitude_band(start).saturating_sub(drop).max(2);
    let alt = match band {
        2 => 1000.,
        3 => 4000.,
        4 => 10000.,
        _ => 20000.,
    };
    if let Some(e) = w.entity_mut(id) {
        e.altitude_m = alt;
        e.ordered_altitude_m = alt;
    }
    (d, band)
}
fn end_fight(w: &mut World, key: u64, f: &Fight, reason: &str) {
    let d = roll(w, EntityId(key as u32), "9.4.6.end_altitude", 10) as i32 - f.rounds as i32;
    let drop = if d <= 1 {
        2
    } else if d <= 5 {
        1
    } else {
        0
    };
    let band = altitude_band(f.altitude_m).saturating_sub(drop).max(2);
    let alt = match band {
        2 => 1000.,
        3 => 4000.,
        4 => 10000.,
        _ => 20000.,
    };
    for &id in &f.members {
        if w.entity(id).is_some_and(|e| e.alive) {
            let e = w.entity_mut(id).unwrap();
            e.altitude_m = alt;
            e.ordered_altitude_m = alt;
            return_to_base(w, id);
            trace(
                w,
                id,
                "9.4.6.ended",
                json!({"fight":key,"rounds":f.rounds,"reason":reason,"altitude_d10_modified":d,"altitude_band":band}),
            );
        }
    }
    w.rules.aircombat.as_mut().unwrap().fights.remove(&key);
}
fn gun_attack(
    w: &mut World,
    attacker: EntityId,
    target: EntityId,
    gun: f64,
    purpose: &str,
) -> Result<bool, String> {
    release_allowed(w, attacker)?;
    let a = w.entity(attacker).ok_or("missing attacker")?.clone();
    let t = w.entity(target).ok_or("missing target")?.clone();
    if gun <= 0. {
        return Ok(false);
    }
    let pk = gun_probability(gun, profile(&t.class)?["damage"].as_f64().unwrap())?;
    let d100 = roll(w, attacker, purpose, 100);
    let hit = d100 as f64 <= pk * 100.;
    // Complete source adjudication stays with the referee; owning shooter sees
    // its target contact, never the hidden target name/class or truth ID.
    w.emit(EventKind::RulesEvent {domain:"aircombat".into(),rule:"9.4.2.gun_table".into(),side:Some(a.side),unit:Some(attacker),contact:w.find_contact(a.side,target),detail:json!({"d100":d100,"probability":pk,"gun":gun,"target_damage":profile(&t.class)?["damage"],"hit":hit}).to_string()},Visibility::Referee);
    trace(
        w,
        attacker,
        "9.4.2.gun_attack",
        json!({"contact":w.find_contact(a.side,target),"d100":d100,"hit":hit}),
    );
    if hit {
        let identity = format!("{purpose}:attacker:{}", attacker.0);
        let optional = super::expanded_damage2026::aircraft_hit(w, target, &identity);
        if !optional || w.entity(target).is_some_and(|e| e.hp <= 0.) {
            crate::damage::sink(w, target);
        }
    }
    Ok(hit)
}
pub fn step(w: &mut World) {
    if !active(w) {
        return;
    }
    let live: BTreeSet<_> = w
        .entities
        .values()
        .filter(|e| e.alive)
        .map(|e| e.id)
        .collect();
    w.rules
        .aircombat
        .as_mut()
        .unwrap()
        .evasions
        .retain(|target, missiles| {
            missiles.retain(|m| live.contains(m));
            live.contains(target) && !missiles.is_empty()
        });
    let pending: Vec<_> = w
        .entities
        .values()
        .filter(|e| {
            e.alive
                && e.kind == crate::types::PlatformKind::Aircraft
                && e.fire_on
                && !in_dogfight(w, e.id)
        })
        .filter_map(|e| e.assigned.map(|c| (e.id, c)))
        .collect();
    for (id, contact) in pending {
        let _ = apply(w, id, &Action::Declare { contact });
    }
    let fights: Vec<_> = w
        .rules
        .aircombat
        .as_ref()
        .unwrap()
        .fights
        .iter()
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    for (key, mut f) in fights {
        f.members
            .retain(|id| w.entity(*id).is_some_and(|e| e.alive));
        let turn = tables()["airborne_asw"]["turn_seconds"].as_i64().unwrap();
        let until = (w.tick.div_euclid(turn) + 1) * turn;
        for id in &f.members {
            w.rules
                .aircombat
                .as_mut()
                .unwrap()
                .surface_prohibited_until
                .insert(*id, until);
        }
        let sides: BTreeSet<_> = f
            .members
            .iter()
            .map(|id| w.entity(*id).unwrap().side.index())
            .collect();
        if sides.len() < 2 {
            end_fight(w, key, &f, "opposition_gone");
            continue;
        }
        for id in &f.members {
            let e = w.entity_mut(*id).unwrap();
            e.pos = f.center;
            e.altitude_m = f.altitude_m;
            e.ordered_altitude_m = f.altitude_m;
        }
        if w.tick < f.next_round {
            w.rules.aircombat.as_mut().unwrap().fights.insert(key, f);
            continue;
        }
        f.rounds += 1;
        f.next_round += 60;
        let mut order = f.members.clone();
        let mut scores = BTreeMap::new();
        let mut unavailable = None;
        for &id in &order {
            let e = w.entity(id).unwrap();
            let maneuver=if f.rounds==1&&starting::surprised(w,id) {
                maneuver_base(w,id,true).map(starting::reduced_maneuver)
            } else {target_maneuver(w,id)};
            let values = maneuver.and_then(|mr| {
                super::aviation2026::entity_source_speed(
                    w,
                    id,
                    e.altitude_m,
                    super::aviation2026::Throttle::Fmp,
                )
                .map(|speed| (mr, starting::round_speed(w,id,f.rounds,speed)))
            });
            let (mr, speed) = match values {
                Ok(v) => v,
                Err(reason) => {
                    unavailable = Some((id, reason));
                    break;
                }
            };
            scores.insert(id, (mr, speed));
        }
        if let Some((id, reason)) = unavailable {
            blocked(w, id, "9.4.round_blocked", &reason);
            f.rounds -= 1;
            w.rules.aircombat.as_mut().unwrap().fights.insert(key, f);
            continue;
        }
        order.sort_by(|a, b| {
            scores[b]
                .0
                .total_cmp(&scores[a].0)
                .then_with(|| scores[b].1.total_cmp(&scores[a].1))
                .then_with(|| a.cmp(b))
        });
        let mut start = 0;
        while start < order.len() {
            let mut end = start + 1;
            while end < order.len()
                && scores[&order[end]].0 == scores[&order[start]].0
                && scores[&order[end]].1 == scores[&order[start]].1
            {
                end += 1;
            }
            if end - start > 1 {
                let mut rng = w.rng.derive(Rng::hash_tag(
                    w.tick,
                    key as u32,
                    &format!("9.4.initiative.coin.{}.{}", f.rounds, order[start].0),
                ));
                let mut draws = Vec::new();
                for i in (start + 1..end).rev() {
                    let sides = i - start + 1;
                    let result = (rng.next_f64() * sides as f64).floor() as usize;
                    draws.push((sides, result + 1));
                    order.swap(i, start + result);
                }
                w.emit(EventKind::RulesEvent {domain:"aircombat".into(),rule:"9.4.initiative".into(),side:None,unit:None,contact:None,detail:json!({"fight":key,"round":f.rounds,"draws":draws,"source":"equal MR/FMP flip a coin; multiway ties use unbiased procedural permutation"}).to_string()},Visibility::Referee);
            }
            start = end;
        }
        for id in order {
            if !w.entity(id).is_some_and(|e| e.alive) {
                continue;
            }
            if f.rounds==1 && starting::surprised(w,id) {continue;}
            let side = w.entity(id).unwrap().side;
            let enemies: Vec<_> = f
                .members
                .iter()
                .copied()
                .filter(|t| w.entity(*t).is_some_and(|e| e.alive && e.side != side))
                .collect();
            let chosen = w
                .rules
                .aircombat
                .as_ref()
                .unwrap()
                .targets
                .get(&id)
                .copied()
                .filter(|id| enemies.contains(id));
            let Some(target) = chosen.or_else(|| enemies.iter().copied().min()) else {
                continue;
            };
            let own_order = w
                .rules
                .aircombat
                .as_ref()
                .unwrap()
                .orders
                .get(&id)
                .copied()
                .unwrap_or(Order::Attack);
            if own_order == Order::Escape {
                let fastest = enemies.iter().map(|id| scores[id].1).fold(0., f64::max);
                let diff = ((scores[&id].1 - fastest) / 50.).trunc() as i32;
                let d10 = roll(w, id, &format!("9.4.5.escape.{key}.{}", f.rounds), 10);
                let escaped = d10 as i32 + diff >= 6;
                trace(
                    w,
                    id,
                    "9.4.5.escape",
                    json!({"d10":d10,"speed_modifier":diff,"escaped":escaped}),
                );
                if escaped {
                    w.rules
                        .aircombat
                        .as_mut()
                        .unwrap()
                        .escaped_from
                        .entry(id)
                        .or_default()
                        .extend(enemies.iter().copied());
                    let (alt_die, band) = departing_altitude(
                        w,
                        id,
                        f.altitude_m,
                        f.rounds,
                        &format!("9.4.6.escape_altitude.{key}.{}", f.rounds),
                    );
                    f.members.retain(|x| *x != id);
                    return_to_base(w, id);
                    trace(
                        w,
                        id,
                        "9.4.6.escape_position",
                        json!({"altitude_d10_modified":alt_die,"altitude_band":band,"must_return":true}),
                    );
                }
                continue;
            }
            if own_order != Order::Attack {
                continue;
            }
            if let Err(reason) = release_allowed(w, id) {
                blocked(w, id, "14.9.attack_blocked", &reason);
                continue;
            }
            let p = profile(&w.entity(id).unwrap().class).unwrap();
            let d100 = roll(w, id, &format!("9.4.1.position.{key}.{}", f.rounds), 100);
            let mut result = position_result(
                d100,
                scores[&id].0,
                scores[&target].0,
                scores[&id].1,
                scores[&target].1,
                p["helmet"].as_u64().unwrap() as u8,
            )
            .unwrap();
            let astern=starting::astern(w,id,f.rounds);
            let modifier=(astern*tables()["position"]["maneuver_diff_percent"].as_f64().unwrap()).round() as i32;
            result.gun+=modifier;result.aam+=modifier;
            result.round = f.rounds;
            trace(
                w,
                id,
                "9.4.1.position",
                json!({"d100":d100,"gun_position":result.gun,"aam_position":result.aam,"round":f.rounds}),
            );
            if starting::has_episode(w,id) {
                w.emit(EventKind::RulesEvent{domain:"aircombat".into(),rule:"9.4.7.position_inputs".into(),side:Some(side),unit:Some(id),contact:None,
                    detail:json!({"fight":key,"round":f.rounds,"attacker_mr":scores[&id].0,"target_mr":scores[&target].0,"attacker_speed_kts":scores[&id].1,"target_speed_kts":scores[&target].1,"astern_maneuver_difference":astern}).to_string()},Visibility::Referee);
            }
            w.rules
                .aircombat
                .as_mut()
                .unwrap()
                .positions
                .insert(id, result.clone());
            let gun = p["gun"].as_f64().unwrap();
            if result.gun < 81 || gun == 0. || !starting::gun_allowed(w,id) {
                continue;
            }
            if w.day_night == crate::types::DayNight::Night && scores[&target].0 > 0.5 {
                blocked(
                    w,
                    id,
                    "9.4.8.gun_blocked",
                    "guns forbidden at night against maneuverable targets",
                );
                continue;
            }
            let defense = if f.rounds==1&&starting::surprised(w,target) {Some(0.0)} else {
                profile(&w.entity(target).unwrap().class).unwrap()["defensive_gun"].as_f64()};
            let defensive = match defense {
                Some(v) => {
                    v * if matches!(
                        w.rules.aircombat.as_ref().unwrap().orders.get(&target),
                        Some(Order::Defend | Order::Escape)
                    ) {
                        0.5
                    } else {
                        1.
                    }
                }
                None => {
                    blocked(
                        w,
                        target,
                        "9.4.3.defensive_guns_blocked",
                        "defensive gun rating not verified",
                    );
                    continue;
                }
            };
            let defensive = if super::expanded_damage2026::aircraft_profile(w, target, 0., false)
                .is_ok_and(|p| p.offensive)
            {
                defensive
            } else {
                0.
            };
            // Defensive-gun kill rolls occur even when the primary gun shot
            // kills the defender: the source explicitly makes them simultaneous.
            let defense_result = if defensive > 0. {
                Some((
                    roll(
                        w,
                        target,
                        &format!("9.4.3.defensive.{key}.{}", f.rounds),
                        100,
                    ),
                    gun_probability(defensive, p["damage"].as_f64().unwrap()),
                ))
            } else {
                None
            };
            if let Some((_, Err(reason))) = &defense_result {
                blocked(w, target, "9.4.3.defensive_guns_blocked", reason);
                continue;
            }
            if let Err(reason) =
                gun_attack(w, id, target, gun, &format!("9.4.2.gun.{key}.{}", f.rounds))
            {
                blocked(w, id, "9.4.2.gun_blocked", &reason);
            }
            if let Some((die, Ok(probability))) = defense_result {
                if die as f64 <= probability * 100. {
                    let identity =
                        format!("9.4.3.defensive.{key}.{}:defender:{}", f.rounds, target.0);
                    let optional = super::expanded_damage2026::aircraft_hit(w, id, &identity);
                    if !optional || w.entity(id).is_some_and(|e| e.hp <= 0.) {
                        crate::damage::sink(w, id);
                    }
                }
            }
        }
        let finish = if f.rounds >= 9 {
            true
        } else if f.rounds == 3 || f.rounds == 6 {
            let threshold = if f.rounds == 3 { 2 } else { 4 };
            let returning = |side: crate::Side| {
                u8::from(f.members.iter().any(|id| {
                    w.entity(*id).is_some_and(|e| e.side == side)
                        && w.rules.aircombat.as_ref().unwrap().must_rtb.contains(id)
                }))
            };
            let a =
                roll(w, EntityId(key as u32), "9.4.6.end_blue", 6) + returning(crate::Side::Blue);
            let b = roll(w, EntityId(key as u32), "9.4.6.end_red", 6) + returning(crate::Side::Red);
            trace(
                w,
                f.members.first().copied().unwrap_or(EntityId(0)),
                "9.4.6.end_roll",
                json!({"d6":[a,b],"threshold":threshold}),
            );
            a <= threshold || b <= threshold
        } else {
            false
        };
        if finish {
            end_fight(w, key, &f, "source_end_roll");
        } else {
            w.rules.aircombat.as_mut().unwrap().fights.insert(key, f);
        }
    }
}

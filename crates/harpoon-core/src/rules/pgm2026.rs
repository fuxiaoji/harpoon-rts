//! §9.6.8: registered source PGM families, common physical release and stock.
//! Player admission uses allocated briefing knowledge; truth stays terminal-only.
use crate::db::{Guidance, Targets, WeaponDef, WeaponFit, WeaponKind};
use crate::rules::{aircombat2026 as ac, aviation2026 as av};
use crate::types::PlatformKind;
use crate::{Database, EntityId, EventKind, Rng, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
mod placement;
pub mod salh;
pub mod controlled;
use super::firecontrol2026 as fc;
use ac::budget;

pub fn tables() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut data: Value = serde_json::from_str(include_str!("../../../../data/rules/pgm-2026.json"))
            .expect("verified PGM source data");
        for family in [controlled::command_sarh::tables(), controlled::eoir1::tables()] {
            for (id, row) in family["weapons"].as_object().expect("controlled source weapon registry") {
                assert!(data["weapons"].get(id).is_none(), "source PGM weapon IDs must be unique across families");
                data["weapons"][id] = row.clone();
            }
            if let Some(fits)=family["fits"].as_object() {
                for (id,row) in fits {
                    assert!(data["fits"].get(id).is_none(),"source experimental carrier IDs must be unique");
                    data["fits"][id]=row.clone();
                }
            }
        }
        data
    })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub enabled_fits: BTreeSet<String>,
    pub briefings: BTreeMap<String, Briefing>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub countermeasures: BTreeMap<String, Countermeasures>,
    #[serde(default, skip_serializing_if="BTreeMap::is_empty")]
    pub sections: BTreeMap<String, Section>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section { pub source:String, pub members:Vec<String>, pub briefing:String }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Briefing {
    pub source: String,
    pub target: String,
    pub aimpoint: String,
    pub known_kind: PlatformKind,
    pub known_motion: Motion,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub moving_lead: Option<MovingLead>,
    pub allowed_sides: Vec<Side>,
    /// A controlled experiment may explicitly supply RF quality. Coordinates
    /// alone never create this input, and a scenario cannot supply dice/results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fcs: Option<AuthoredFcs>,
}
/// Explicit RTS conversion for a frozen own estimate and physical footprint.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovingLead {
    pub source: String,
    pub observed_at: i64,
    pub course_deg: f64,
    pub speed_kts: f64,
    pub footprint_nm: f64,
}
fn moving_aim(w: &World, actor: &crate::world::Entity, b: &Briefing, point: Vec2, speed: f64) -> Result<Vec2,String> {
    if b.known_motion == Motion::Stationary { return Ok(point); }
    let m=b.moving_lead.as_ref().ok_or("9.6.8 moving target own lead and footprint undeclared")?;
    if !m.source.starts_with("scenario_assumption:") || m.observed_at > w.tick || m.observed_at < 0
        || !m.course_deg.is_finite() || !m.speed_kts.is_finite() || m.speed_kts < 0.
        || !m.footprint_nm.is_finite() || m.footprint_nm <= 0. || !speed.is_finite() || speed <= m.speed_kts {
        return Err("9.6.8 moving target authored lead parameters invalid".into());
    }
    let v=Vec2::from_bearing(m.course_deg).scaled(m.speed_kts/3600.);
    let initial=point.add(v.scaled((w.tick-m.observed_at) as f64));
    let r=initial.sub(actor.pos); let weapon=speed/3600.;
    let aa=weapon*weapon-v.x*v.x-v.y*v.y;
    let rv=r.x*v.x+r.y*v.y;
    let time=(rv+(rv*rv+aa*(r.x*r.x+r.y*r.y)).sqrt())/aa;
    if !time.is_finite() || time <= 0. { return Err("9.6.8 no finite moving intercept".into()); }
    Ok(initial.add(v.scaled(time)))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredFcs {
    pub source: String,
    pub quality: fc::Quality,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlacementEvidence {
    pub quality: fc::Quality,
    pub provenance: Value,
    pub placement: Option<fc::Placement>,
    pub exemption: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motion {
    Stationary,
    Moving,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Countermeasures {
    pub source: String,
    pub generation: u8,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Configuration,
    #[serde(default)]
    pub salh: salh::State,
    #[serde(default)]
    pub eoir: controlled::eoir1::State,
    #[serde(default)]
    pub command_sarh: controlled::command_sarh::State,
    pub targets: BTreeMap<String, EntityId>,
    pub drafts: BTreeMap<EntityId, Vec<Request>>,
    pub pending: BTreeMap<EntityId, Prepared>,
    pub flights: BTreeMap<EntityId, Flight>,
    pub outcomes: BTreeMap<EntityId, Value>,
    #[serde(default, skip_serializing_if="BTreeMap::is_empty")]
    pub section_placements: BTreeMap<String, PlacementEvidence>,
    #[serde(default, skip_serializing_if="BTreeMap::is_empty")]
    pub prh_criticals: BTreeMap<EntityId, (EntityId, String, String)>,
    pub own_status: BTreeMap<EntityId, String>,
    pub sequence: BTreeMap<EntityId, u64>,
    #[serde(skip)]
    committing: Option<(EntityId, String)>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub weapon: String,
    pub rounds: u16,
    pub briefing: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prepared {
    pub actor: EntityId,
    pub side: Side,
    pub class: String,
    pub request: Request,
    pub aim: Vec2,
    pub target: EntityId,
    pub known_kind: PlatformKind,
    pub accepted_at: i64,
    #[serde(default)]
    pub identity: String,
    #[serde(default)]
    pub fcs: Option<PlacementEvidence>,
    #[serde(default)]
    pub designation: Option<super::detection2026::laser::BoundDesignation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Flight {
    pub identity: String,
    pub actor: EntityId,
    pub side: Side,
    pub weapon: String,
    pub target: EntityId,
    pub known_kind: PlatformKind,
    pub briefing: String,
    pub aim: Vec2,
    pub launched_at: i64,
    pub launch_pos: Vec2,
    pub launch_altitude_m: f64,
    pub total_range_nm: f64,
    pub travelled_nm: f64,
    pub last_tick: i64,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub moving_footprint_nm: Option<f64>,
    #[serde(default)]
    pub fcs: Option<PlacementEvidence>,
    #[serde(default)]
    pub salh: Option<salh::Guidance>,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub eoir: Option<controlled::eoir1::Guidance>,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub command_sarh: Option<controlled::command_sarh::Guidance>,
}
fn active(w: &World) -> bool {
    w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full && w.rules.pgm.is_some()
}
fn trace(w: &mut World, actor: EntityId, rule: &str, detail: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "pgm2026".into(),
            rule: rule.into(),
            side: w.entity(actor).map(|e| e.side),
            unit: Some(actor),
            contact: None,
            detail: detail.to_string(),
        },
        Visibility::Referee,
    );
}
pub fn configure_database(db: &mut Database) {
    for (id, row) in tables()["weapons"].as_object().unwrap() {
        db.weapons.insert(
            id.clone(),
            WeaponDef {
                id: id.clone(),
                name: row["name"].as_str().unwrap().into(),
                kind: if row["ordnance_kind"] == "guided_missile"
                    || controlled::command_sarh::is_weapon(id) {
                    WeaponKind::Ascm
                } else { WeaponKind::Bomb },
                targets: Targets {
                    surface: true,
                    land: true,
                    ..Default::default()
                },
                range_nm: row["range_nm"].as_f64().unwrap(),
                min_range_nm: row["min_range_nm"].as_f64().unwrap(),
                speed_kts: row["speed_kts"].as_f64().unwrap(),
                enable_range_nm: 0.,
                pk: 0.,
                salvo: 1,
                reload_s: 0.,
                damage: row["damage_points"].as_f64().unwrap(),
                guidance: Guidance::BallisticGun,
                ir: false,
                directors: 0,
                rule_data: Some(super::damage_runtime::WeaponDamage {
                    ship_dp: row["damage_points"].as_f64(),
                    extra: row["additional_criticals"].as_str().unwrap_or("").into(),
                    penetration_cm: row["penetration_cm"].as_f64(),
                    profile: if salh::is_weapon(id) {
                        "glide"
                    } else {
                        "level"
                    }
                    .into(),
                    bomb_kind: "LDGP".into(),
                    source: row["source"].as_str().unwrap().into(),
                    min_launch_altitude_m: row["min_launch_altitude_m"].as_f64().unwrap_or_else(|| {
                        if controlled::eoir1::is_weapon(id) || controlled::command_sarh::is_weapon(id) { 0. } else { 7500. }
                    }),
                    range_nm: row["range_nm"].as_f64(),
                    min_range_nm: row["min_range_nm"].as_f64(),
                    speed_kts: row["speed_kts"].as_f64(),
                    ..Default::default()
                }),
            },
        );
    }
    for (id, row) in tables()["fits"].as_object().unwrap() {
        let Some(mut plane) = db.platforms.get(row["base"].as_str().unwrap()).cloned() else {
            continue;
        };
        plane.id = id.clone();
        plane.name = row["name"].as_str().unwrap().into();
        plane.weapons = row["weapons"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(weapon, count)| WeaponFit {
                weapon: weapon.clone(),
                mounts: 1,
                ammo: count.as_u64().unwrap() as u16,
            })
            .collect();
        db.platforms.insert(id.clone(), plane);
    }
}
pub fn external_store_weight(class: &str, weapon: &str) -> Option<f64> {
    if let Some(weight) = controlled::external_store_weight(class, weapon) { return Some(weight); }
    let host = &tables()["salh_author_external_fits"][class]["weapons"][weapon];
    (!host.is_null() && salh::is_weapon(weapon))
        .then(|| tables()["weapons"][weapon]["hang_weight_kg"].as_f64())
        .flatten()
        .filter(|v| v.is_finite() && *v > 0.)
}
pub fn source_weapon(def: &WeaponDef) -> bool {
    tables()["weapons"].get(&def.id).is_some()
}
/// Only the synchronous, validated source commit sets this non-serialized
/// capability; ordinary Fire/auto/naked launch and restored input cannot.
pub fn launch_authorized(w: &World, actor: EntityId, def: &WeaponDef) -> bool {
    active(w)
        && w.rules
            .pgm
            .as_ref()
            .and_then(|s| s.committing.as_ref())
            .is_some_and(|(owner, weapon)| *owner == actor && weapon == &def.id)
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    if w.rules.profile != super::damage_runtime::RuleProfile::Rts2026Full {
        return Ok(());
    }
    let config = w.context.pgm_2026.clone().unwrap_or_default();
    if w.context.pgm_2026.is_some() && config.source.trim().is_empty() {
        return Err("9.6.8 PGM configuration source required".into());
    }
    for class in &config.enabled_fits {
        if tables()["fits"][class].is_null() {
            return Err("9.6.8 unverified exact PGM fit".into());
        }
    }
    let mut state = State {
        config,
        ..Default::default()
    };
    salh::configure(w, &mut state)?;
    controlled::configure(w, &mut state)?;
    for (key, b) in &state.config.briefings {
        if key.is_empty()
            || b.source.trim().is_empty()
            || b.allowed_sides.is_empty()
            || !matches!(b.known_kind, PlatformKind::Ship | PlatformKind::Base)
        {
            return Err("9.6.8 incomplete allocated briefing source/type".into());
        }
        if let Some(fcs) = &b.fcs {
            if !fcs.source.starts_with("scenario_assumption:") || fcs.quality == fc::Quality::NA {
                return Err("9.6.8 authored RF quality requires explicit scenario_assumption source and Poor/Fair/Good, never coordinates or NA".into());
            }
        }
        let matches: Vec<_> = w
            .entities
            .values()
            .filter(|e| e.name == b.target)
            .map(|e| e.id)
            .collect();
        if matches.len() != 1 {
            return Err("9.6.8 briefing Referee binding requires one named scenario target".into());
        }
        let aim = w
            .context
            .aviation_2026
            .as_ref()
            .and_then(|c| c.surface_aimpoints.get(&b.aimpoint))
            .ok_or("9.6.8 briefing needs a cited fixed published aimpoint")?;
        if aim.source.trim().is_empty()
            || !aim.controlled_range
            || (b.known_motion == Motion::Stationary && !aim.fixed_target)
            || !aim.point.x.is_finite()
            || !aim.point.y.is_finite()
        {
            return Err("9.6.8 invalid fixed briefing aimpoint".into());
        }
        state.targets.insert(key.clone(), matches[0]);
    }
    let mut section_members=BTreeSet::new();
    for section in state.config.sections.values() {
        if !section.source.starts_with("scenario_assumption:") || !(3..=4).contains(&section.members.len()) || !state.config.briefings.contains_key(&section.briefing) {return Err("9.6.8 section requires authored 3-4 members and allocated briefing".into());}
        let mut side=None;
        for name in &section.members {
            if !section_members.insert(name.clone()) {return Err("9.6.8 aircraft cannot belong to multiple sections".into());}
            let members:Vec<_>=w.entities.values().filter(|e|e.name==*name && e.alive && e.kind==PlatformKind::Aircraft).collect();
            if members.len()!=1 || side.is_some_and(|x|x!=members[0].side) {return Err("9.6.8 section needs distinct exact same-side aircraft".into());}
            side=Some(members[0].side);
        }
        if !state.config.briefings[&section.briefing].allowed_sides.contains(&side.unwrap()) {return Err("9.6.8 section briefing not allocated to aircraft side".into());}
    }
    for cm in state.config.countermeasures.values() {
        if cm.source.trim().is_empty() || cm.generation > 4 {
            return Err("9.6.8 source countermeasure generation0–4 required".into());
        }
    }
    for e in w
        .entities
        .values()
        .filter(|e| state.config.enabled_fits.contains(&e.class))
    {
        let row = &tables()["fits"][&e.class];
        let expected: BTreeMap<String, u16> =
            serde_json::from_value(row["weapons"].clone()).unwrap();
        let mut physical = BTreeMap::<String, u16>::new();
        for m in &e.weapons {
            *physical.entry(m.weapon.clone()).or_default() += u16::from(m.mounts) * m.ammo;
        }
        let f = w
            .rules
            .aviation
            .as_ref()
            .and_then(|s| s.flights.get(&e.id))
            .ok_or("9.6.8 exact PGM internal flight/load failed initialization")?;
        if physical != expected
            || f.internal_weapon_rounds != expected
            || !f.external_weapon_rounds.is_empty()
            || (f.payload_fraction * f.spec.max_payload_kg - row["payload_kg"].as_f64().unwrap())
                .abs()
                > 1e-8
        {
            return Err("9.6.8 source physical inventory/internal mass mismatch".into());
        }
        if w.rules.nuclear.as_ref().is_some_and(|s| {
            s.fits
                .get(&e.id)
                .is_some_and(|m| m.keys().any(|key| !tables()["weapons"][key].is_null()))
        }) {
            return Err("9.6.8 this exact Mk84 fit has no nuclear counterpart".into());
        }
    }
    w.rules.pgm = Some(state);
    Ok(())
}
fn validate_platform(w: &World, actor: EntityId, weapon: &str) -> Result<(), String> {
    let a = w.entity(actor).ok_or("9.6.8 own live aircraft required")?;
    if controlled::eoir1::is_weapon(weapon) || (controlled::command_sarh::is_weapon(weapon) && !controlled::command_sarh::tables()["weapons"][weapon]["guidance"].as_str().is_some_and(|s| s.starts_with("PRH/"))) {
        w.context.controlled_pgm_los_2026.as_ref()
            .ok_or("9.6.8 controlled optical clearance volume must be explicitly declared")?.validate()?;
    }
    if !super::unmanned2026::can_attack(w, actor)
        || !super::expanded_damage2026::platform_weapons_operational(w, actor)
        || !super::expanded_damage2026::weapon_operational(w, actor, weapon)?
        || a.ext.get("nuclear.mission_abort") == Some(&1.)
    {
        return Err("9.6.8 own aircraft/weapon cannot attack".into());
    }
    ac::surface_attack_gate(w, actor)?;
    super::nuclear2026::release_gate(w, actor, weapon)?;
    if w.rules
        .nuclear
        .as_ref()
        .is_some_and(|s| s.fits.get(&actor).is_some_and(|m| m.contains_key(weapon)))
    {
        return Err(
            "9.6.8 exact conventional source weapon cannot acquire a nuclear counterpart".into(),
        );
    }
    Ok(())
}
fn check_stock(
    w: &World,
    actor: EntityId,
    weapon: &str,
    rounds: u16,
    external: bool,
) -> Result<(), String> {
    let a = w.entity(actor).ok_or("9.6.8 own live aircraft required")?;
    let stock: u32 = a
        .weapons
        .iter()
        .filter(|m| m.weapon == weapon)
        .map(|m| u32::from(m.mounts) * u32::from(m.ammo))
        .sum();
    let flight = w
        .rules
        .aviation
        .as_ref()
        .and_then(|s| s.flights.get(&actor))
        .ok_or("9.6.8 source internal/external flight state missing")?;
    let inventory = if external {
        &flight.external_weapon_rounds
    } else {
        &flight.internal_weapon_rounds
    };
    if stock < u32::from(rounds) || inventory.get(weapon).copied().unwrap_or(0) < rounds {
        return Err("9.6.8 physical/internal or external source stock unavailable".into());
    }
    Ok(())
}
fn ready(w: &World, actor: EntityId, request: &Request) -> Result<Prepared, String> {
    if controlled::eoir1::is_weapon(&request.weapon) {
        return controlled::eoir1::ready(w, actor, request);
    }
    if controlled::command_sarh::is_weapon(&request.weapon) {
        return controlled::command_sarh::ready(w, actor, request);
    }
    if salh::is_weapon(&request.weapon) {
        return salh::ready(w, actor, request);
    }
    if !active(w) {
        return Err("9.6.8 source PGM requires explicit v4".into());
    }
    let state = w.rules.pgm.as_ref().unwrap();
    let a = w
        .entity(actor)
        .filter(|e| e.alive && e.kind == PlatformKind::Aircraft)
        .ok_or("9.6.8 own live aircraft required")?;
    if !state.config.enabled_fits.contains(&a.class) {
        return Err("9.6.8 exact PGM fit not enabled".into());
    }
    validate_platform(w, actor, &request.weapon)?;
    let row = &tables()["weapons"][&request.weapon];
    if row.is_null()
        || row["guidance"] != "I&Sat/1"
        || tables()["fits"][&a.class]["weapons"][&request.weapon].is_null()
        || request.rounds == 0
    {
        return Err("9.6.8 exact source guided load/positive count required".into());
    }
    let b = state
        .config
        .briefings
        .get(&request.briefing)
        .filter(|b| b.allowed_sides.contains(&a.side))
        .ok_or("9.6.8 allocated own briefing unavailable")?;
    let aim = av::surface_aimpoint(w, a.side, &b.aimpoint)
        .ok_or("9.6.8 allocated own fixed aimpoint unavailable")?;
    if b.known_motion != Motion::Stationary {
        return Err(
            "9.6.8 SATNAV without a terminal seeker cannot attack an own-known moving target"
                .into(),
        );
    }
    if !ac::satnav_release_allowed(1, a.altitude_m, ac::Bombsight::Advanced, false, false) {
        return Err("9.6.8 SATNAV1 requires source High release and a qualified bombsight; no terminal seeker is fitted".into());
    }
    let distance = a.pos.range_to(aim.point);
    if !distance.is_finite()
        || !a.course_deg.is_finite()
        || distance < row["min_range_nm"].as_f64().unwrap()
        || distance > row["range_nm"].as_f64().unwrap()
        || crate::geom::angle_diff(a.course_deg, a.pos.bearing_to(aim.point)).abs()
            > tables()["off_axis_deg"].as_f64().unwrap()
    {
        return Err("9.6.8 source PGM range/forward30degree envelope exceeded".into());
    }
    check_stock(w, actor, &request.weapon, request.rounds, false)?;
    Ok(Prepared {
        actor,
        side: a.side,
        class: a.class.clone(),
        request: request.clone(),
        aim: aim.point,
        target: state.targets[&request.briefing],
        known_kind: b.known_kind,
        accepted_at: w.tick,
        identity: String::new(),
        fcs: None,
        designation: None,
    })
}
/// Read-only public command preflight; uses only own briefing for admission.
pub fn validate_request(w: &World, side: Side, actor: EntityId, weapon: &str, rounds: u16, briefing: &str) -> Result<(), String> {
    if w.over || crate::exercise::locked(w, side) || !w.entity(actor).is_some_and(|e| e.alive && e.side == side) {
        return Err("9.6.8 guided attack requires active own aircraft".into());
    }
    ready(w, actor, &Request { weapon: weapon.into(), rounds, briefing: briefing.into() }).map(|_| ())
}
pub fn declare(
    w: &mut World,
    actor: EntityId,
    weapon: &str,
    rounds: u16,
    briefing: &str,
) -> Result<(), String> {
    let request = Request {
        weapon: weapon.into(),
        rounds,
        briefing: briefing.into(),
    };
    ready(w, actor, &request)?;
    let s = w.rules.pgm.as_mut().unwrap();
    if s.pending.contains_key(&actor) {
        return Err("9.6.8 a PGM release is already reserved".into());
    }
    let list = s.drafts.entry(actor).or_default();
    if !list.contains(&request) {
        list.push(request)
    }
    Ok(())
}
pub fn before_defense_scan(w: &mut World) {
    if !active(w) {
        return;
    }
    let drafts = std::mem::take(&mut w.rules.pgm.as_mut().unwrap().drafts);
    for (actor, mut variants) in drafts {
        if variants.len() != 1 {
            budget::cancel_actor(w, actor);
            w.rules
                .pgm
                .as_mut()
                .unwrap()
                .own_status
                .insert(actor, "conflicting_declarations".into());
            trace(w, actor, "9.6.8.release_conflict", json!({"consumed":0}));
            continue;
        }
        let request = variants.pop().unwrap();
        match ready(w, actor, &request) {
            Ok(p) => {
                if let Err(reason) = ac::surface::request_defense(w, actor) {
                    trace(w, actor, "9.6.8.release_pending", json!({"reason":reason}));
                    continue;
                }
                w.rules.pgm.as_mut().unwrap().pending.insert(actor, p);
                w.rules
                    .pgm
                    .as_mut()
                    .unwrap()
                    .own_status
                    .insert(actor, "pending".into());
            }
            Err(reason) => {
                trace(
                    w,
                    actor,
                    "9.6.8.release_aborted",
                    json!({"reason":reason,"consumed":0}),
                );
            }
        }
    }
    // The original accepted tick and canonical request remain stable across a
    // wait/save. Old pre-budget pending saves enter the same common barrier.
    let actors: Vec<_> = w
        .rules
        .pgm
        .as_ref()
        .unwrap()
        .pending
        .keys()
        .copied()
        .collect();
    for actor in actors {
        let mut p = w.rules.pgm.as_ref().unwrap().pending[&actor].clone();
        if p.identity.is_empty() {
            p.identity = budget::claim_identity(
                budget::AirAttackKind::Pgm,
                actor,
                p.accepted_at,
                &serde_json::to_value(&p.request).unwrap(),
            );
            w.rules
                .pgm
                .as_mut()
                .unwrap()
                .pending
                .insert(actor, p.clone());
        }
        if let Err(reason)=placement::section_declared(w,&p) {
            cancel_release(w,&p,reason);
            continue;
        }
        let claim = budget::Claim {
            identity: p.identity.clone(),
            actor,
            target: p.target,
            known_kind: p.known_kind,
            kind: budget::AirAttackKind::Pgm,
        };
        if let Err(reason) = budget::submit(w, claim) {
            cancel_release(w, &p, reason);
        }
    }
}
fn cancel_release(w: &mut World, p: &Prepared, reason: String) {
    budget::abort(w, &p.identity);
    w.rules.pgm.as_mut().unwrap().pending.remove(&p.actor);
    w.rules
        .pgm
        .as_mut()
        .unwrap()
        .own_status
        .insert(p.actor, "cancelled".into());
    ac::surface::finish_defense(w, p.actor);
    trace(
        w,
        p.actor,
        "9.6.8.release_aborted",
        json!({"identity":p.identity,"reason":reason,"consumed":0}),
    );
}
/// Provider stage: no stock, internal mass or projectile mutation. All domains
/// offer readiness before root closes the one shared readiness barrier.
pub fn prepare_releases(w: &mut World) {
    if !active(w) {
        return;
    }
    let actors: Vec<_> = w
        .rules
        .pgm
        .as_ref()
        .unwrap()
        .pending
        .keys()
        .copied()
        .collect();
    for actor in actors {
        let old = w.rules.pgm.as_ref().unwrap().pending[&actor].clone();
        match budget::decision(w, &old.identity) {
            budget::Decision::Reserved => (),
            other => {
                cancel_release(w, &old, format!("9.6 common aircraft budget {other:?}"));
                continue;
            }
        }
        let mut current = match ready(w, actor, &old.request) {
            Ok(p) => p,
            Err(reason) => {
                cancel_release(w, &old, reason);
                continue;
            }
        };
        let ac::surface::DefenseGate::Ready { epoch } = ac::surface::defense_gate(w, actor) else {
            continue;
        };
        current.identity = old.identity.clone();
        current.accepted_at = old.accepted_at;
        match placement::qualify(w, &current) {
            Ok(evidence) => current.fcs = Some(evidence),
            Err(reason) => {
                let status = w
                    .rules
                    .pgm
                    .as_mut()
                    .unwrap()
                    .own_status
                    .insert(actor, "source_pending_rf_fcs".into());
                if status.as_deref() != Some("source_pending_rf_fcs") {
                    trace(
                        w,
                        actor,
                        "9.6.8.release_pending",
                        json!({"identity":old.identity,"reason":reason,"consumed":0}),
                    );
                }
                continue;
            }
        }
        if let Err(reason) = placement::section_ready(w,&current,epoch) {
            cancel_release(w,&old,reason);
            continue;
        }
        w.rules.pgm.as_mut().unwrap().pending.insert(actor, current);
        if let Err(reason) = budget::offer_ready(w, &old.identity, epoch) {
            cancel_release(w, &old, reason);
        }
    }
}
/// Commit only a once-only ticket from the common all-provider barrier.
pub fn commit_releases(w: &mut World) {
    if !active(w) {
        return;
    }
    let actors: Vec<_> = w
        .rules
        .pgm
        .as_ref()
        .unwrap()
        .pending
        .keys()
        .copied()
        .collect();
    for actor in actors {
        let old = w.rules.pgm.as_ref().unwrap().pending[&actor].clone();
        let Some(ticket) = budget::take_ticket(w, &old.identity) else {
            continue;
        };
        if ticket.actor != actor || ticket.target != old.target {
            cancel_release(w, &old, "9.6 PGM ticket identity/target mismatch".into());
            continue;
        }
        let mut current = match ready(w, actor, &old.request) {
            Ok(p) => p,
            Err(reason) => {
                cancel_release(w, &old, reason);
                continue;
            }
        };
        if !matches!(
            ac::surface::defense_gate(w, actor),
            ac::surface::DefenseGate::Ready { epoch } if epoch == ticket.epoch
        ) {
            cancel_release(
                w,
                &old,
                "9.6 PGM defense epoch changed after ready barrier".into(),
            );
            continue;
        }
        current.identity = old.identity.clone();
        current.accepted_at = old.accepted_at;
        current.fcs = old.fcs.clone();
        let fcs = match placement::commit(w, &current) {
            Ok(evidence) => evidence,
            Err(reason) => {
                cancel_release(w, &old, reason);
                continue;
            }
        };
        trace(
            w,
            actor,
            "9.6.8.rf_placement",
            json!({"identity":current.identity,"evidence":fcs}),
        );
        let def = w.db.weapons[&current.request.weapon].clone();
        let a = w.entity(actor).unwrap().clone();
        let serial = {
            let s = w.rules.pgm.as_mut().unwrap();
            let n = s.sequence.entry(actor).or_default();
            *n += 1;
            *n
        };
        let mut released = 0u16;
        for round in 0..current.request.rounds {
            w.rules.pgm.as_mut().unwrap().committing = Some((actor, def.id.clone()));
            let spawned =
                crate::weapons::launch(w, actor, &def, Some(current.target), Some(current.aim));
            w.rules.pgm.as_mut().unwrap().committing = None;
            let Some(id) = spawned else { break };
            // launch emits the normal release and the sole aviation internal
            // mass notification. Correct its generic0.03nm/5m birth to this
            // source domain's actual release anchor before physical advance.
            let course = a.pos.bearing_to(current.aim);
            let e = w.entity_mut(id).unwrap();
            e.pos = a.pos;
            e.altitude_m = a.altitude_m;
            e.ordered_altitude_m = a.altitude_m;
            e.course_deg = course;
            e.ordered_course_deg = course;
            if let Some(m) = w
                .entity_mut(actor)
                .unwrap()
                .weapons
                .iter_mut()
                .find(|m| m.weapon == def.id && m.ammo > 0)
            {
                m.ammo -= 1;
            }
            let identity = format!("9.6.8:{}:{}:{serial}:{round}", actor.0, w.tick);
            let f = Flight {
                identity,
                actor,
                side: a.side,
                weapon: def.id.clone(),
                target: current.target,
                known_kind: current.known_kind,
                briefing: current.request.briefing.clone(),
                aim: current.aim,
                launched_at: w.tick,
                launch_pos: a.pos,
                launch_altitude_m: a.altitude_m,
                total_range_nm: a.pos.range_to(current.aim),
                travelled_nm: 0.,
                last_tick: w.tick,
                moving_footprint_nm: w.rules.pgm.as_ref().unwrap().config.briefings[&current.request.briefing].moving_lead.as_ref().filter(|_| w.rules.pgm.as_ref().unwrap().config.briefings[&current.request.briefing].known_motion == Motion::Moving).map(|m| m.footprint_nm),
                fcs: Some(fcs.clone()),
                salh: salh::guidance(w, &current),
                eoir: controlled::eoir1::guidance(w, &current),
                command_sarh: controlled::command_sarh::guidance(w, &current),
            };
            w.rules.pgm.as_mut().unwrap().flights.insert(id, f);
            released += 1;
        }
        if released > 0 {
            salh::note_release(w, &current);
        }
        w.rules.pgm.as_mut().unwrap().pending.remove(&actor);
        w.rules.pgm.as_mut().unwrap().own_status.insert(
            actor,
            if released > 0 {
                "released"
            } else {
                "cancelled"
            }
            .into(),
        );
        ac::surface::finish_defense(w, actor);
        if released > 0 {
            budget::finish(w, &current.identity);
        } else {
            budget::abort(w, &current.identity);
        }
        trace(
            w,
            actor,
            "9.6.8.source_release",
            json!({"weapon":def.id,"rounds":released,"requested":current.request.rounds,"position":a.pos,"altitude_m":a.altitude_m,"speed_kts":def.speed_kts,"briefing":current.request.briefing,"source":tables()["rts"]["source"]}),
        );
    }
}
/// Compatibility for an isolated provider test; the normal simulation uses
/// prepare(all) -> close_ready(once) -> commit(all), never this wrapper.
pub fn resolve_releases(w: &mut World) {
    prepare_releases(w);
    budget::close_ready(w);
    commit_releases(w);
}
fn target_data(w: &World, f: &Flight) -> Result<(char, f64, u8), String> {
    let t = w
        .entity(f.target)
        .filter(|e| e.alive && e.kind == f.known_kind)
        .ok_or("9.6.8 Referee target state/category differs from the captured briefing")?;
    if f.moving_footprint_nm.is_none() && (t.speed_kts != 0.
        || t.pos.range_to(f.aim)
            > tables()["rts"]["fixed_point_tolerance_nm"]
                .as_f64()
                .unwrap())
    {
        return Err("9.6.8 Referee actual target moved from the authorized SATNAV point; source placement unresolved".into());
    }
    let (size, armor) = if t.kind == PlatformKind::Base {
        let d = super::ground2026::source_target(w, t.id)?;
        (d.size, d.horizontal_armor)
    } else {
        let p = t
            .rule_data
            .as_ref()
            .or_else(|| super::damage_runtime::platform(&t.class))
            .ok_or("9.6.8 Referee physical target size/armor missing")?;
        (
            p.size.chars().next().ok_or("9.6.8 physical size missing")?,
            p.deck_cm,
        )
    };
    if tables()["physical_size_vsmall"][size.to_string()]
        .as_bool()
        .is_none()
        || !armor.is_finite()
        || armor < 0.
    {
        return Err("9.6.8 source physical size/armor unresolved".into());
    }
    let state = w.rules.pgm.as_ref().unwrap();
    let cm = if let Some(cm) = state.config.countermeasures.get(&t.name) {
        cm.generation
    } else if t.kind == PlatformKind::Base {
        if w.weather.jam.iter().any(|j| *j > 0.)
            || w.entities
                .values()
                .any(|e| e.alive && super::electronic_warfare::emitting(e))
            || w.rules
                .ew
                .as_ref()
                .is_some_and(|s| s.objects.values().any(|o| o.effective(w.tick)))
        {
            return Err(
                "9.6.8 active EW applicability/PGM countermeasure source not qualified".into(),
            );
        }
        0
    } else {
        return Err("9.6.8 source target PGM countermeasure fit unresolved".into());
    };
    Ok((size, armor, cm))
}
fn terminal(w: &mut World, id: EntityId, f: &Flight) -> Result<(), String> {
    let mut rng = w
        .rng
        .derive(Rng::hash_tag(f.launched_at, f.actor.0, &f.identity));
    let die = 1 + (rng.next_f64() * 100.) as u8;
    terminal_result(
        w,
        id,
        f,
        ac::PgmGuidance::Satnav,
        1,
        false,
        die,
        Value::Null,
    )
}
fn hold_terminal(w: &mut World, id: EntityId, f: &Flight, reason: String) {
    let Some(snapshot) = w.entity(id).cloned() else {
        return;
    };
    w.rules
        .pending_adjudications
        .entry(id)
        .or_insert(super::damage_runtime::PendingAdjudication {
            evidence: vec![],
            tick: w.tick,
            rule: "9.6.8.source_pgm".into(),
            target: f.target,
            projectile: snapshot,
        });
    trace(
        w,
        f.actor,
        "9.6.8.terminal_pending",
        json!({"projectile":id,"reason":reason,"physical_release_already_committed":true}),
    );
}
pub fn resolve_deferred_guidance(w: &mut World) {
    if !active(w) { return; }
    super::damage_runtime::begin(w);
    salh::resolve_deferred(w);
    controlled::resolve_deferred(w);
    super::surface2026::flush_land_hits(w);
    let queued = std::mem::take(&mut w.rules.pgm.as_mut().unwrap().prh_criticals);
    for (_, (target, sensor, identity)) in queued {
        if w.entity(target).is_some_and(|e|e.alive && e.hp > 0.) {
            if let Err(reason) = super::ground2026::apply_damage(w, target, 0., 1, &identity) {
                trace(w, target, "9.6.8.prh_radar_critical_pending", json!({"identity":identity,"reason":reason}));
            } else {
                trace(w, target, "9.6.8.prh_radar_critical", json!({"identity":identity,"sensor":sensor}));
            }
        } else {
            trace(w, target, "9.6.8.prh_radar_already_destroyed", json!({"identity":identity}));
        }
    }
    super::damage_runtime::finish(w);
}
fn prh_preflight(w: &World, target_id: EntityId, briefing: &str) -> Result<String, String> {
    let sensor = w.rules.pgm.as_ref().and_then(|s|s.command_sarh.config.as_ref())
        .and_then(|c|c.prh_radars.get(briefing)).ok_or("9.6.8 PRH exact authored radar binding absent")?.clone();
    let target = w.entity(target_id).filter(|e|e.alive).ok_or("9.6.8 PRH target missing")?;
    let facility = super::ground2026::source_target(w, target_id)?;
    if target.kind != PlatformKind::Base || facility.effect != "radar_destroyed" || target.sensors.len() != 1
        || target.sensors[0].sensor != sensor || !w.db.sensors.get(&sensor).is_some_and(|d|d.kind == crate::db::SensorKind::Radar) {
        return Err("9.6.8 PRH requires one exact source radar facility; other target scope pending".into());
    }
    if !crate::sensors::radar_emitting(target) || !crate::sensors::radar_operational(&w.db, target)
        || !super::ground2026::operational(w,target_id)
        || !super::expanded_damage2026::sensor_operational(w,target_id,&sensor)? {
        return Err("9.6.8 PRH radar no longer emitting/operational; shutdown-memory consequence source pending".into());
    }
    Ok(sensor)
}
fn terminal_result(
    w: &mut World,
    id: EntityId,
    f: &Flight,
    guidance: ac::PgmGuidance,
    generation: u8,
    los_lost: bool,
    d100: u8,
    extra: Value,
) -> Result<(), String> {
    placement::terminal(&f.fcs)?;
    if let Some(radius) = f.moving_footprint_nm {
        let target = w.entity(f.target).filter(|e|e.alive && e.kind==f.known_kind).ok_or("9.6.8 moving target unavailable at terminal")?;
        if target.pos.range_to(f.aim) > radius {
            let result=json!({"identity":f.identity,"projectile":id,"weapon":f.weapon,"target":f.target,"d100":d100,"hit":false,"geometric_miss":true,"footprint_nm":radius,"source":"scenario_assumption: frozen straight-line intercept and authored footprint"});
            w.rules.pgm.as_mut().unwrap().outcomes.insert(id,result.clone());
            trace(w,f.actor,"9.6.8.source_terminal",result);
            if let Some(e)=w.entity_mut(id) { e.alive=false; e.hp=0.; }
            return Ok(());
        }
    }
    let (size, armor, cm) = target_data(w, f)?;
    let prh_sensor = if matches!(guidance, ac::PgmGuidance::Prh) {
        Some(prh_preflight(w, f.target, &f.briefing)?)
    } else { None };
    let small = tables()["physical_size_vsmall"][size.to_string()]
        .as_bool()
        .unwrap();
    let probability = ac::pgm_probability(guidance, generation, small, cm, los_lost)?;
    let row = &tables()["weapons"][&f.weapon];
    let pen = row["penetration_cm"].as_f64().unwrap();
    if armor > pen {
        return Err("9.6.8 protected-target nonpenetrating damage mapping pending".into());
    }
    let hit = f64::from(d100) <= probability * 100.;
    if hit && f.known_kind == PlatformKind::Base && row["additional_criticals"].as_str().is_some_and(|v| !v.is_empty()) {
        return Err("9.6.8 source extra criticals on Base require common facility consumer; original die retained".into());
    }
    let (dp, damage_d6) = if hit && prh_sensor.is_some() {
        let sides = row["damage_roll"]["sides"].as_u64();
        let add = row["damage_roll"]["add"].as_u64();
        let criticals = row["automatic_radar_criticals"].as_u64();
        if sides != Some(6) || add != Some(2) || criticals != Some(1) {
            return Err("9.6.8 PRH damage roll/automatic radar critical source row unsupported; pending".into());
        }
        let mut r = w.rng.derive(Rng::hash_tag(f.launched_at, f.actor.0, &format!("{}:prh_damage", f.identity)));
        let d = 1 + (r.next_f64() * sides.unwrap() as f64) as u8;
        (f64::from(d) + add.unwrap() as f64, Some(d))
    } else { (row["damage_points"].as_f64().unwrap(), None) };
    let def = w.db.weapons[&f.weapon].clone();
    if hit {
        if f.known_kind == PlatformKind::Base {
            w.rules.aaw.as_mut().unwrap().surface.land_hits.insert(
                id,
                super::surface2026::LandHit {
                    target: f.target,
                    weapon: def.clone(),
                    shooter_side: f.side,
                    damage: dp,
                },
            );
        } else {
            super::damage_runtime::queue(
                w,
                f.target,
                &def,
                false,
                Some(0.),
                Some(f.side),
                Some(f.identity.clone()),
            );
        }
    }
    if hit {
        if let Some(sensor) = prh_sensor {
            w.rules.pgm.as_mut().unwrap().prh_criticals.insert(id, (f.target, sensor, format!("{}:prh_radar", f.identity)));
        }
    }
    let mut result = json!({"damage_d6":damage_d6,"identity":f.identity,"projectile":id,"weapon":f.weapon,"target":f.target,"physical_size":size,"probability":probability,"countermeasures_generation":cm,"d100":d100,"hit":hit,"damage_points_per_hit":dp,"penetration_cm":pen,"flight_range_nm":f.travelled_nm,"speed_kts":row["speed_kts"],"guidance":row["guidance"],"rf_placement":f.fcs});
    if !extra.is_null() {
        result[if f.eoir.is_some() {"eoir"} else if f.command_sarh.is_some() {"command_sarh"} else {"salh"}] = extra;
        result["los_lost"] = json!(los_lost);
    }
    w.rules
        .pgm
        .as_mut()
        .unwrap()
        .outcomes
        .insert(id, result.clone());
    trace(w, f.actor, "9.6.8.source_terminal", result);
    if let Some(e) = w.entity_mut(id) {
        e.alive = false;
        e.hp = 0.;
    }
    Ok(())
}
/// true only claims this module's registered, still physically alive bomb.
pub fn advance(w: &mut World, id: EntityId) -> bool {
    if !active(w)
        || !w
            .entity(id)
            .is_some_and(|e| e.alive && e.projectile.is_some())
    {
        return false;
    }
    let Some(mut f) = w.rules.pgm.as_ref().unwrap().flights.get(&id).cloned() else {
        return false;
    };
    if w.entity(id)
        .and_then(|e| e.projectile.as_ref())
        .is_none_or(|p| p.weapon != f.weapon)
    {
        return false;
    }
    if w.rules.pgm.as_ref().unwrap().outcomes.contains_key(&id) {
        return true;
    }
    let elapsed = (w.tick - f.last_tick).max(0) as f64;
    if elapsed == 0. {
        return true;
    }
    let speed = tables()["weapons"][&f.weapon]["speed_kts"]
        .as_f64()
        .unwrap();
    let position = w.entity(id).unwrap().pos;
    let remaining = position.range_to(f.aim);
    let distance = (speed * elapsed / 3600.).min(remaining);
    let next = if distance >= remaining {
        f.aim
    } else {
        position.advanced(position.bearing_to(f.aim), distance)
    };
    f.travelled_nm += distance;
    f.last_tick = w.tick;
    if let Some(e) = w.entity_mut(id) {
        e.pos = next;
        e.speed_kts = speed;
        e.altitude_m = f.launch_altitude_m * (1. - f.travelled_nm / f.total_range_nm).max(0.);
        if let Some(p) = e.projectile.as_mut() {
            p.live_s += elapsed;
        }
    }
    w.rules.pgm.as_mut().unwrap().flights.insert(id, f.clone());
    if distance >= remaining {
        if f.eoir.is_some() {
            controlled::eoir1::queue_arrival(w, id, &f);
            return true;
        }
        if f.command_sarh.is_some() {
            controlled::command_sarh::queue_arrival(w, id, &f);
            return true;
        }
        if f.salh.is_some() {
            salh::queue_arrival(w, id, &f);
            return true;
        }
        if let Err(reason) = terminal(w, id, &f) {
            let snapshot = w.entity(id).unwrap().clone();
            w.rules.pending_adjudications.entry(id).or_insert(
                super::damage_runtime::PendingAdjudication {
                    evidence: vec![],
                    tick: w.tick,
                    rule: "9.6.8.source_pgm".into(),
                    target: f.target,
                    projectile: snapshot,
                },
            );
            trace(
                w,
                f.actor,
                "9.6.8.terminal_pending",
                json!({"projectile":id,"reason":reason,"physical_release_already_committed":true}),
            );
        }
    }
    true
}
pub fn own_view(w: &World, actor: EntityId) -> Option<Value> {
    if !active(w) {
        return None;
    }
    let a = w.entity(actor)?;
    let s = w.rules.pgm.as_ref()?;
    if !s.config.enabled_fits.contains(&a.class) && !s.salh.fits.contains_key(&actor)
        && !s.eoir.fits.contains_key(&actor) && !s.command_sarh.fits.contains_key(&actor) {
        return None;
    }
    let briefings: Vec<_> = s
        .config
        .briefings
        .iter()
        .filter(|(_, b)| {
            b.allowed_sides.contains(&a.side)
                && av::surface_aimpoint(w, a.side, &b.aimpoint).is_some()
        })
        .map(|(key, _)| key.clone())
        .collect();
    Some(
        json!({"fit":a.class,"briefings":briefings,"status":s.own_status.get(&actor),"pending":s.pending.get(&actor).map(|p|json!({"weapon":p.request.weapon,"rounds":p.request.rounds,"briefing":p.request.briefing,"declared_at":p.accepted_at}))}),
    )
}

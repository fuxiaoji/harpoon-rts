//! §8.5.3 contact patterns, .2 per-Tac phase budgets and .3 Hedgehog.
//! Experimental mechanics are explicit authored data, never an Annex alias.
use super::{active, tables, Configuration, MountState, Quality, UnitState};
use crate::db::{Guidance, WeaponKind};
use crate::types::PlatformKind;
use crate::{ContactId, EntityId, EventKind, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fuzing {
    Contact,
    Hydrostatic,
    ContactHydrostatic,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub source: String,
    pub fuzing: Fuzing,
    pub range_nm: f64,
    pub max_depth_band_index: usize,
    pub bombs: u16,
    pub contact_percent: Option<f64>,
    pub major_percent: Option<f64>,
    pub minor_percent: Option<f64>,
    pub damage_contact: Option<f64>,
    pub damage_major: Option<f64>,
    pub damage_minor: Option<f64>,
    pub loaded_capacity_patterns: u16,
    pub attacks_per_tactical_turn: u8,
    pub hedgehog: bool,
    pub paired_mounts: bool,
    pub physical_arcs: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirePhase {
    Planned,
    Reaction,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Unit {
    name: String,
    class: String,
    owner: Side,
    hardware: UnitState,
    model: Model,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    units: BTreeMap<EntityId, BTreeMap<String, Unit>>,
    requests: Vec<Request>,
    turns: BTreeMap<String, Turn>,
    patterns: BTreeMap<String, Pattern>,
    next: u64,
}
impl State {
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
            && self.requests.is_empty()
            && self.turns.is_empty()
            && self.patterns.is_empty()
            && self.next == 0
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Request {
    actor: EntityId,
    contact: ContactId,
    weapon: String,
    physical: usize,
    phase: FirePhase,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Turn {
    target: EntityId,
    quality: Quality,
    evidence: Value,
    planned: u8,
    reaction: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Pattern {
    pub(super) id: String,
    pub(super) actor: EntityId,
    pub(super) owner: Side,
    pub(super) target: EntityId,
    pub(super) contact: ContactId,
    pub(super) weapon: String,
    pub(super) physical: usize,
    pub(super) mount_index: usize,
    pub(super) phase: FirePhase,
    pub(super) epoch: i64,
    pub(super) tick: i64,
    pub(super) aim: Vec2,
    pub(super) quality: Quality,
    pub(super) evidence: Value,
    pub(super) model: Model,
    pub(super) sea_state: u8,
    pub(super) result: Option<Value>,
}
#[derive(Clone, Debug)]
struct Permit {
    target: EntityId,
    aim: Vec2,
    quality: Quality,
    evidence: Value,
    sea: u8,
    epoch: i64,
}
fn state(w: &World) -> Result<&State, String> {
    w.rules
        .asw_weapons
        .as_ref()
        .map(|s| &s.projectors.advanced)
        .ok_or_else(|| "8.5.3 contact source runtime absent".into())
}
fn unit<'a>(s: &'a State, actor: EntityId, weapon: &str) -> Result<&'a Unit, String> {
    s.units
        .get(&actor)
        .and_then(|m| m.get(weapon))
        .ok_or_else(|| "8.5.3 explicit physical contact/multiple projector fit absent".into())
}
fn turn_key(actor: EntityId, weapon: &str, physical: usize, epoch: i64) -> String {
    format!("{}:{weapon}:{physical}:{epoch}", actor.0)
}
fn f(key: &str) -> f64 {
    tables()["projector"]["advanced"][key]
        .as_f64()
        .expect("verified source projector constant")
}
pub(super) fn validate_model(m: &Model) -> Result<(), String> {
    let contact = matches!(m.fuzing, Fuzing::Contact | Fuzing::ContactHydrostatic);
    let hydro = matches!(m.fuzing, Fuzing::Hydrostatic | Fuzing::ContactHydrostatic);
    let valid_pair = |p: Option<f64>, d: Option<f64>, needed: bool| match (p, d, needed) {
        (Some(p), Some(d), true) => {
            p.is_finite() && (0. ..=100.).contains(&p) && d.is_finite() && d > 0.
        }
        (None, None, false) => true,
        _ => false,
    };
    if !m.source.starts_with("scenario_assumption:")
        || !m.range_nm.is_finite()
        || m.range_nm <= 0.
        || m.max_depth_band_index == 0
        || m.max_depth_band_index > 7
        || m.bombs == 0
        || m.loaded_capacity_patterns == 0
        || !(1..=3).contains(&m.attacks_per_tactical_turn)
        || m.physical_arcs.is_empty()
        || (m.hedgehog && !contact)
        || !valid_pair(m.contact_percent, m.damage_contact, contact)
        || !valid_pair(m.major_percent, m.damage_major, hydro)
        || !valid_pair(m.minor_percent, m.damage_minor, hydro)
        || m.major_percent
            .zip(m.minor_percent)
            .is_some_and(|(a, b)| a > b)
        || m.contact_percent
            .zip(m.major_percent)
            .is_some_and(|(a, b)| a > b)
    {
        return Err("8.5.3 explicit laboratory source/finite fuzing thresholds/DP/capacity and1..3 attacks required".into());
    }
    for arc in &m.physical_arcs {
        super::super::super::aaw2026::arc_allows(arc, 90.)?;
    }
    Ok(())
}
/// Pure constructor; parent publishes only after every domain validates.
pub(super) fn initialize(w: &World, c: &Configuration) -> Result<State, String> {
    let mut s = State::default();
    for (weapon, fit) in c
        .projectors
        .iter()
        .filter(|(_, f)| f.experiment.as_ref().is_some_and(|m| !m.paired_mounts))
    {
        let m = fit.experiment.as_ref().unwrap();
        validate_model(m)?;
        let d =
            w.db.weapons
                .get(weapon)
                .ok_or("8.5.3 experimental physical weapon definition absent")?;
        if fit.model != "scenario_assumption"
            || !fit.source.starts_with("scenario_assumption:")
            || fit.units.is_empty()
            || d.kind != WeaponKind::AswRocket
            || d.guidance != Guidance::AswBallistic
            || c.standoffs.contains_key(weapon)
        {
            return Err("8.5.3 experimental mechanics cannot override an authoritative model or stand-off carrier".into());
        }
        for (name, u) in &fit.units {
            let found: Vec<_> = w.entities.values().filter(|e| e.name == *name).collect();
            if found.len() != 1 {
                return Err("8.5.3 exact unique original projector unit name required".into());
            }
            let e = found[0];
            let a = e
                .weapons
                .get(u.mount_index)
                .ok_or("8.5.3 original aggregate mount absent")?;
            if !e.alive
                || e.kind != PlatformKind::Ship
                || u.source.trim().is_empty()
                || u.reload.is_some()
                || a.weapon != *weapon
                || a.mounts == 0
                || e.weapons.iter().filter(|a| a.weapon == *weapon).count() != 1
                || u.mounts.len() != usize::from(a.mounts)
                || m.physical_arcs.len() != usize::from(a.mounts)
            {
                return Err("8.5.3 exact live original ship/mount allocation and unresolved reload required".into());
            }
            let mut mounts = BTreeMap::new();
            for p in &u.mounts {
                if p.physical >= usize::from(a.mounts)
                    || p.loaded_patterns > m.loaded_capacity_patterns
                    || m.physical_arcs.get(p.physical) != Some(&p.arc)
                    || mounts.contains_key(&p.physical)
                {
                    return Err(
                        "8.5.3 exact physical source arc/loaded capacity and unique mount required"
                            .into(),
                    );
                }
                mounts.insert(
                    p.physical,
                    MountState {
                        physical: p.physical,
                        arc: p.arc.clone(),
                        loaded_patterns: p.loaded_patterns,
                        reserve_patterns: p.reserve_patterns,
                        used_epoch: None,
                    },
                );
            }
            let hardware = UnitState {
                source: u.source.clone(),
                model: fit.model.clone(),
                fit_source: fit.source.clone(),
                mount_index: u.mount_index,
                mounts,
            };
            if super::stock(&hardware.mounts)? != a.ammo {
                return Err(
                    "8.5.3 original loaded+reserve must equal real aggregate ammunition".into(),
                );
            }
            let live = super::surviving_mounts(w, e.id, weapon, &hardware)?;
            super::verify_component_stock(w, e.id, weapon, &hardware, &live)?;
            s.units.entry(e.id).or_default().insert(
                weapon.clone(),
                Unit {
                    name: name.clone(),
                    class: e.class.clone(),
                    owner: e.side,
                    hardware,
                    model: m.clone(),
                },
            );
        }
    }
    Ok(s)
}
pub fn configured(w: &World, actor: EntityId, weapon: &str) -> bool {
    active(w) && state(w).is_ok_and(|s| s.units.get(&actor).is_some_and(|m| m.contains_key(weapon)))
}
pub fn is_action(a: &super::super::depth_charge::Action) -> bool {
    matches!(
        a,
        super::super::depth_charge::Action::ProjectorPattern { .. }
    )
}
fn request(actor: EntityId, a: &super::super::depth_charge::Action) -> Result<Request, String> {
    let super::super::depth_charge::Action::ProjectorPattern {
        contact,
        weapon,
        physical,
        phase,
    } = a
    else {
        return Err("8.5.3 explicit projector pattern action required".into());
    };
    Ok(Request {
        actor,
        contact: *contact,
        weapon: weapon.clone(),
        physical: *physical,
        phase: *phase,
    })
}
fn preflight(w: &World, s: &State, r: &Request) -> Result<Permit, String> {
    if !active(w) || w.over {
        return Err("8.5.3 active Full source profile required".into());
    }
    let e = w
        .entity(r.actor)
        .filter(|e| e.alive)
        .ok_or("8.5.3 original live launcher absent")?;
    if crate::exercise::locked(w, e.side)
        || !super::super::super::unmanned2026::can_attack(w, r.actor)
        || !super::super::super::expanded_damage2026::platform_weapons_operational(w, r.actor)
        || !super::super::super::expanded_damage2026::weapon_operational(w, r.actor, &r.weapon)?
        || !super::super::super::damage_runtime::weapons_operational(e)
        || e.systems.fire_control <= 0.
    {
        return Err("8.5.3 actual control/power/crew/weapon unavailable".into());
    }
    if r.phase == FirePhase::Reaction
        && !super::super::super::unmanned2026::can_reaction_fire(w, r.actor)
    {
        return Err("2.5 HITL unmanned units cannot attack in the Reaction Fire phase".into());
    }
    super::super::super::nuclear2026::release_gate(w, r.actor, &r.weapon)?;
    let u = unit(s, r.actor, &r.weapon)?;
    if e.name != u.name || e.class != u.class || e.side != u.owner {
        return Err("8.5.3 original body/class/owner binding changed".into());
    }
    let h = &u.hardware;
    let a = e
        .weapons
        .get(h.mount_index)
        .filter(|a| a.weapon == r.weapon)
        .ok_or("8.5.3 aggregate mount binding changed")?;
    let surviving = super::surviving_mounts(w, r.actor, &r.weapon, h)?;
    if super::surviving_stock(h, &surviving)? != a.ammo {
        return Err("8.5.3 aggregate/physical pattern ammunition conflict".into());
    }
    super::verify_component_stock(w, r.actor, &r.weapon, h, &surviving)?;
    let p = h
        .mounts
        .get(&r.physical)
        .ok_or("8.5.3 selected original physical projector absent")?;
    let operational = super::super::super::expanded_damage2026::physical_weapon_mounts(
        w,
        r.actor,
        h.mount_index,
    )?;
    if !surviving.contains(&r.physical)
        || !operational.contains(&r.physical)
        || p.loaded_patterns == 0
    {
        return Err("8.5.3 actual selected physical projector unavailable or unloaded".into());
    }
    let sea = super::sea_state(w)?;
    if f64::from(sea)
        >= tables()["projector"]["sea_state_exclusive_upper"]
            .as_f64()
            .unwrap()
    {
        return Err("8.5.3 no projector fire in sea state7+".into());
    }
    let c = w.sides[e.side.index()]
        .contacts
        .get(&r.contact)
        .filter(|c| !c.stale && !c.bearing_only)
        .ok_or("8.5.3 original own located contact required")?;
    let epoch = w.tick.div_euclid(f("tactical_seconds") as i64);
    let old = s
        .turns
        .get(&turn_key(r.actor, &r.weapon, r.physical, epoch));
    if old.is_some_and(|t| t.target != c.about) {
        return Err(
            "8.5.3 same-mount same-Tac retargeting solution priority is not source-established"
                .into(),
        );
    }
    let (planned, reaction) = old.map_or((0, 0), |t| (t.planned, t.reaction));
    let planned_limit = if u.model.attacks_per_tactical_turn == 1 {
        1
    } else {
        u.model.attacks_per_tactical_turn - 1
    };
    let reaction_limit = if u.model.attacks_per_tactical_turn == 1 {
        0
    } else {
        1
    };
    if planned + reaction >= u.model.attacks_per_tactical_turn
        || match r.phase {
            FirePhase::Planned => reaction > 0 || planned >= planned_limit,
            FirePhase::Reaction => reaction >= reaction_limit,
        }
    {
        return Err("8.5.3.2 original Tactical-Turn Planned/Reaction pattern budget exhausted or phase closed".into());
    }
    let mut candidates = Vec::new();
    if let Some(aaw) = &w.rules.aaw {
        for t in aaw
            .firecontrol
            .tracks
            .values()
            .filter(|t| t.observer == r.actor && t.target == c.about)
        {
            let Some(report) = &t.report else {
                continue;
            };
            if !super::attack_sonar::weapon_matches(w, r.actor, &r.weapon, &t.sensor) {
                continue;
            }
            let Some(aim) = report
                .measured_position
                .filter(|p| p.x.is_finite() && p.y.is_finite())
            else {
                continue;
            };
            let Ok(solution) = super::super::super::firecontrol2026::own_projector_solution(
                w, r.actor, c.about, &t.sensor,
            ) else {
                continue;
            };
            if solution.quality == Quality::NA
                || e.pos.range_to(aim) > u.model.range_nm
                || !super::super::super::aaw2026::arc_allows(
                    &p.arc,
                    e.pos.bearing_to(aim) - e.course_deg,
                )?
            {
                continue;
            }
            candidates.push((solution.quality,aim,json!({"source":"own_native_same_receiver","sensor":t.sensor,"observer":r.actor,"at":report.at,"report":report,"motion":t.motion,"solution":solution,"dedicated_attack_sonar":super::attack_sonar::fcs_allowed(w,r.actor,c.about,&t.sensor,report)})));
        }
    }
    let (quality, aim, evidence) = candidates.into_iter().max_by_key(|v| v.0).ok_or(
        "8.5.3 actual source-qualified active native measured FCS/range/selected arc required",
    )?;
    Ok(Permit {
        target: c.about,
        aim,
        quality: old.map_or(quality, |t| t.quality),
        evidence: old.map_or(evidence.clone(), |t| t.evidence.clone()),
        sea,
        epoch,
    })
}
pub fn validate_action(
    w: &World,
    actor: EntityId,
    a: &super::super::depth_charge::Action,
) -> Result<(), String> {
    preflight(w, state(w)?, &request(actor, a)?)?;
    Ok(())
}
pub fn control(
    w: &mut World,
    actor: EntityId,
    a: super::super::depth_charge::Action,
) -> Result<(), String> {
    let r = request(actor, &a)?;
    preflight(w, state(w)?, &r)?;
    w.rules
        .asw_weapons
        .as_mut()
        .unwrap()
        .projectors
        .advanced
        .requests
        .push(r);
    Ok(())
}
fn reject(w: &mut World, r: &Request, reason: &str) {
    let side = w.entity(r.actor).map(|e| e.side);
    if let Some(side) = side {
        w.emit(
            EventKind::RulesEvent {
                domain: "asw_weapons2026".into(),
                rule: "8.5.3.request_rejected".into(),
                side: Some(side),
                unit: Some(r.actor),
                contact: Some(r.contact),
                detail:
                    json!({"weapon":r.weapon,"physical":r.physical,"phase":r.phase,"reason":reason})
                        .to_string(),
            },
            Visibility::Side(side),
        );
    }
}
fn commit(w: &mut World, s: &mut State, r: &Request) -> Result<(), String> {
    let permit = preflight(w, s, r)?;
    let u = unit(s, r.actor, &r.weapon)?.clone();
    let next = s
        .next
        .checked_add(1)
        .ok_or("8.5.3 advanced pattern sequence exhausted")?;
    let id = format!(
        "8.5.3:contact:{}:{}:{}:{}:{next}",
        r.actor.0, u.hardware.mount_index, r.physical, permit.epoch
    );
    super::super::super::expanded_damage2026::spend_physical_weapon_stock(
        w,
        r.actor,
        &r.weapon,
        u.hardware.mount_index,
        r.physical,
        1,
    )?;
    w.entity_mut(r.actor).unwrap().weapons[u.hardware.mount_index].ammo -= 1;
    let mount = s
        .units
        .get_mut(&r.actor)
        .unwrap()
        .get_mut(&r.weapon)
        .unwrap()
        .hardware
        .mounts
        .get_mut(&r.physical)
        .unwrap();
    mount.loaded_patterns -= 1;
    mount.used_epoch = Some(permit.epoch);
    let t = s
        .turns
        .entry(turn_key(r.actor, &r.weapon, r.physical, permit.epoch))
        .or_insert_with(|| Turn {
            target: permit.target,
            quality: permit.quality,
            evidence: permit.evidence.clone(),
            planned: 0,
            reaction: 0,
        });
    match r.phase {
        FirePhase::Planned => t.planned += 1,
        FirePhase::Reaction => t.reaction += 1,
    };
    s.next = next;
    s.patterns.insert(
        id.clone(),
        Pattern {
            id: id.clone(),
            actor: r.actor,
            owner: u.owner,
            target: permit.target,
            contact: r.contact,
            weapon: r.weapon.clone(),
            physical: r.physical,
            mount_index: u.hardware.mount_index,
            phase: r.phase,
            epoch: permit.epoch,
            tick: w.tick,
            aim: permit.aim,
            quality: permit.quality,
            evidence: permit.evidence,
            model: u.model.clone(),
            sea_state: permit.sea,
            result: None,
        },
    );
    super::super::trace(
        w,
        r.actor,
        "8.5.3.pattern_committed",
        json!({"attack_id":id,"weapon":r.weapon,"physical":r.physical,"fire_phase":r.phase,"stock_tokens":1,"bombs":u.model.bombs,"quality":permit.quality,"source":"8.5.3/.2/.3; authored laboratory hardware"}),
    );
    let name = w.entity(r.actor).unwrap().name.clone();
    let weapon_name = w.db.weapons[&r.weapon].name.clone();
    w.emit(
        EventKind::WeaponsRelease {
            side: u.owner,
            shooter: r.actor,
            shooter_name: name,
            weapon: weapon_name,
            count: 1,
        },
        Visibility::Side(u.owner),
    );
    Ok(())
}
pub(super) fn adjudicate(w: &mut World, p: &Pattern, one_of_required_pair: bool) -> Value {
    let mut out = json!({"attack_id":p.id,"weapon":p.weapon,"model":"scenario_assumption","physical":p.physical,"aggregate_mount_index":p.mount_index,"epoch":p.epoch,"bombs":p.model.bombs,"fire_phase":p.phase,"quality":p.quality,"source_fcs":p.evidence,"raw_d100":null,"thresholds":{"contact":null,"major":null,"minor":null},"severity":"miss","dp":null,"contact_hits":0,"contact_outcomes":[],"damage_queued":false,"reason":null});
    if p.quality == Quality::Poor {
        out["reason"] = json!("8.5.3 Poor FCS automatic miss");
        return out;
    }
    let die = super::super::die(w, p.actor, p.tick, &format!("{}:pattern_d100", p.id), 100);
    out["raw_d100"] = json!(die);
    // True target state is inspected only after the complete physical debit.
    let Some(t) = w.entity(p.target) else {
        out["severity"] = json!("pending");
        out["reason"] = json!("original target physics unavailable after expenditure");
        return out;
    };
    if !t.alive || t.kind != PlatformKind::Sub {
        out["reason"] = json!("no actual live submarine at physical adjudication");
        return out;
    }
    out["physical_speed_kts"] = json!(t.speed_kts);
    out["physical_depth_m"] = json!(t.depth_m);
    let band = match super::super::super::detection2026::depth_band(t.depth_m) {
        Ok(b) if b > 0 => b,
        _ => {
            out["severity"] = json!("pending");
            out["reason"] = json!("source ASW modifier matrix has no Surface column");
            return out;
        }
    };
    out["physical_depth_band"] = json!(band);
    if band > p.model.max_depth_band_index {
        out["reason"] = json!("target below explicit source maximum depth; spent pattern misses");
        return out;
    }
    if t.depth_m
        > tables()["depth_charge"]["matrix_deep_i_max_m"]
            .as_f64()
            .unwrap()
    {
        out["severity"] = json!("pending");
        out["reason"] = json!("matrix stops at Deep I, no collapsed Deep+ permission");
        return out;
    }
    let col = if band <= 2 {
        0
    } else if band <= 4 {
        1
    } else if band <= 6 {
        2
    } else {
        3
    };
    let thresholds = match super::super::projector_thresholds(
        [
            p.model.contact_percent,
            p.model.major_percent,
            p.model.minor_percent,
        ],
        p.quality,
        t.speed_kts.abs(),
        col,
        p.sea_state,
        one_of_required_pair,
    ) {
        Ok(v) => v,
        Err(reason) => {
            if !reason.contains("attack not possible") {
                out["severity"] = json!("pending");
            }
            out["reason"] = json!(reason);
            return out;
        }
    };
    out["thresholds"] = json!({"contact":p.model.contact_percent.map(|_|thresholds[0]),"major":p.model.major_percent.map(|_|thresholds[1]),"minor":p.model.minor_percent.map(|_|thresholds[2])});
    if p.model.contact_percent.is_some() && u32::from(die) <= thresholds[0] {
        let count = if !p.model.hedgehog {
            1
        } else if f64::from(die) <= f64::from(thresholds[0]) * f("hedgehog_three_fraction") {
            3
        } else if f64::from(die) <= f64::from(thresholds[0]) * f("hedgehog_two_fraction") {
            2
        } else {
            1
        };
        let dp = p.model.damage_contact.unwrap();
        out["severity"] = json!("contact");
        out["dp"] = json!(dp * f64::from(count));
        out["contact_hits"] = json!(count);
        let mut outcomes = Vec::new();
        for n in 1..=count {
            match super::super::super::damage_runtime::queue_asw_projector_contact(
                w,
                p.actor,
                p.target,
                &p.weapon,
                dp,
                &format!("{}:contact:{n}", p.id),
            ) {
                Ok(o) => {
                    if o.damage_queued {
                        out["damage_queued"] = json!(true);
                    }
                    outcomes.push(json!(o));
                }
                Err(reason) => {
                    out["reason"] =
                        json!(format!("direct contact common consumer pending: {reason}"));
                }
            }
        }
        out["contact_outcomes"] = json!(outcomes);
    } else {
        let hit = if p.model.major_percent.is_some() && u32::from(die) <= thresholds[1] {
            Some((true, p.model.damage_major.unwrap()))
        } else if p.model.minor_percent.is_some() && u32::from(die) <= thresholds[2] {
            Some((false, p.model.damage_minor.unwrap()))
        } else {
            None
        };
        if let Some((major, dp)) = hit {
            out["severity"] = json!(if major { "major" } else { "minor" });
            out["dp"] = json!(dp);
            match super::super::super::damage_runtime::queue_projector(
                w, p.actor, p.target, &p.weapon, dp, major, &p.id,
            ) {
                Ok(()) => out["damage_queued"] = json!(true),
                Err(reason) => out["reason"] = json!(reason),
            }
        }
    }
    out
}
pub(super) fn resolve(w: &mut World) {
    if !active(w) {
        return;
    }
    let Some(mut s) = w
        .rules
        .asw_weapons
        .as_mut()
        .map(|a| std::mem::take(&mut a.projectors.advanced))
    else {
        return;
    };
    let mut groups: BTreeMap<(EntityId, String, usize), BTreeMap<String, Request>> =
        BTreeMap::new();
    for r in std::mem::take(&mut s.requests) {
        groups
            .entry((r.actor, r.weapon.clone(), r.physical))
            .or_default()
            .insert(serde_json::to_string(&r).unwrap(), r);
    }
    for choices in groups.values() {
        if choices.len() != 1 {
            for r in choices.values() {
                reject(w,r,"different simultaneous requests on one physical projector conflict; no network phase/target priority");
            }
            continue;
        }
        let r = choices.values().next().unwrap();
        if let Err(reason) = commit(w, &mut s, r) {
            reject(w, r, &reason);
        }
    }
    let ids: Vec<_> = s
        .patterns
        .iter()
        .filter(|(_, p)| p.result.is_none())
        .map(|(id, _)| id.clone())
        .collect();
    for id in ids {
        let p = s.patterns[&id].clone();
        let result = adjudicate(w, &p, false);
        super::super::trace(w, p.actor, "8.5.3.pattern_result", result.clone());
        s.patterns.get_mut(&id).unwrap().result = Some(result);
    }
    w.rules.asw_weapons.as_mut().unwrap().projectors.advanced = s;
}
pub(super) fn own_view(w: &World, actor: EntityId) -> Value {
    let Ok(s) = state(w) else {
        return Value::Null;
    };
    let Some(units) = s.units.get(&actor) else {
        return Value::Null;
    };
    let epoch = w.tick.div_euclid(f("tactical_seconds") as i64);
    let mut out = BTreeMap::new();
    for (weapon, u) in units {
        if !w
            .entity(actor)
            .is_some_and(|e| e.side == u.owner && e.name == u.name && e.class == u.class)
        {
            continue;
        }
        let live = super::surviving_mounts(w, actor, weapon, &u.hardware);
        let consistent = live.as_ref().is_ok_and(|set| {
            super::surviving_stock(&u.hardware, set).ok()
                == w.entity(actor)
                    .and_then(|e| e.weapons.get(u.hardware.mount_index))
                    .map(|a| a.ammo)
                && super::verify_component_stock(w, actor, weapon, &u.hardware, set).is_ok()
        });
        let operational = super::super::super::expanded_damage2026::physical_weapon_mounts(
            w,
            actor,
            u.hardware.mount_index,
        )
        .unwrap_or_default();
        let mounts:Vec<_>=u.hardware.mounts.values().map(|m|{
            let t=s.turns.get(&turn_key(actor,weapon,m.physical,epoch));
            let destroyed=live.as_ref().is_ok_and(|set|!set.contains(&m.physical));
            json!({"physical":m.physical,"arc":m.arc,"loaded_patterns":if destroyed{0}else{m.loaded_patterns},"reserve_patterns":if destroyed{0}else{m.reserve_patterns},"operational":consistent&&operational.contains(&m.physical),"planned_used":t.map_or(0,|t|t.planned),"reaction_used":t.map_or(0,|t|t.reaction),"retained_quality":t.map(|t|t.quality),"reload_pending":!destroyed&&m.loaded_patterns==0&&m.reserve_patterns>0})
        }).collect();
        out.insert(weapon,json!({"source":u.hardware.source,"fit_source":u.hardware.fit_source,"model":"scenario_assumption","experimental_source":u.model.source,"mount_index":u.hardware.mount_index,"stock_unit":"complete_pattern","bombs_per_pattern":u.model.bombs,"mounts":mounts,"reload_policy":null,"attack_sonar":super::attack_sonar::own_view(w,actor,weapon)}));
    }
    json!(out)
}

//! §8.5.3 actual two-mount pattern transaction. The declared physical salvo
//! inventory is spent on each firing mount; the source pattern still rolls once.
use super::advanced::{self, FirePhase, Model};
use super::{active, attack_sonar, tables, Configuration, MountState, Quality, UnitState};
use crate::db::{Guidance, WeaponKind};
use crate::types::PlatformKind;
use crate::{ContactId, EntityId, EventKind, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    units: BTreeMap<EntityId, BTreeMap<String, Unit>>,
    requests: Vec<Request>,
    patterns: BTreeMap<String, Pattern>,
    next: u64,
}
impl State {
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
            && self.requests.is_empty()
            && self.patterns.is_empty()
            && self.next == 0
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Unit {
    name: String,
    class: String,
    owner: Side,
    hardware: UnitState,
    model: Model,
    used_epoch: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Request {
    actor: EntityId,
    contact: ContactId,
    weapon: String,
    physicals: Vec<usize>,
    phase: FirePhase,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Pattern {
    base: advanced::Pattern,
    physicals: Vec<usize>,
    one_of_required_pair: bool,
    result: Option<Value>,
}
struct Permit {
    target: EntityId,
    aim: Vec2,
    quality: Quality,
    evidence: Value,
    sea: u8,
    epoch: i64,
    one_of_required_pair: bool,
}
fn state(w: &World) -> Result<&State, String> {
    w.rules
        .asw_weapons
        .as_ref()
        .map(|s| &s.projectors.paired)
        .ok_or_else(|| "8.5.3 paired runtime absent".into())
}
fn unit<'a>(s: &'a State, actor: EntityId, weapon: &str) -> Result<&'a Unit, String> {
    s.units
        .get(&actor)
        .and_then(|m| m.get(weapon))
        .ok_or_else(|| "8.5.3 explicit actual paired-system fit absent".into())
}
/// All physical allocation rows validate before the parent publishes any fit.
pub(super) fn initialize(w: &World, c: &Configuration) -> Result<State, String> {
    let mut s = State::default();
    for (weapon, fit) in c
        .projectors
        .iter()
        .filter(|(_, f)| f.experiment.as_ref().is_some_and(|m| m.paired_mounts))
    {
        let model = fit.experiment.as_ref().unwrap();
        advanced::validate_model(model)?;
        if model.physical_arcs.len() != 2
            || model.bombs % 2 != 0
            || model.attacks_per_tactical_turn != 1
        {
            return Err("8.5.3 pair requires exactly two explicitly identical original mounts/even complete charge count/one source pattern per Tactical Turn; unestablished groups remain pending".into());
        }
        if fit.model != "scenario_assumption"
            || !fit.source.starts_with("scenario_assumption:")
            || fit.units.is_empty()
            || !w.db.weapons.get(weapon).is_some_and(|d| {
                d.kind == WeaponKind::AswRocket && d.guidance == Guidance::AswBallistic
            })
            || c.standoffs.contains_key(weapon)
        {
            return Err("8.5.3 explicit authored paired source and actual projectile hardware required; named models cannot be overridden".into());
        }
        for (name, u) in &fit.units {
            let found: Vec<_> = w.entities.values().filter(|e| e.name == *name).collect();
            if found.len() != 1 {
                return Err("8.5.3 paired unit name must bind exactly one original body".into());
            }
            let e = found[0];
            let a = e
                .weapons
                .get(u.mount_index)
                .ok_or("8.5.3 paired original aggregate mount absent")?;
            if !e.alive
                || e.kind != PlatformKind::Ship
                || u.source.trim().is_empty()
                || u.reload.is_some()
                || a.weapon != *weapon
                || a.mounts != 2
                || u.mounts.len() != 2
                || e.weapons.iter().filter(|a| a.weapon == *weapon).count() != 1
            {
                return Err("8.5.3 exact live two-mount original pair/explicit per-mount salvo allocation and unresolved reload required".into());
            }
            let mut mounts = BTreeMap::new();
            for p in &u.mounts {
                if p.physical >= 2
                    || p.loaded_patterns > model.loaded_capacity_patterns
                    || model.physical_arcs.get(p.physical) != Some(&p.arc)
                    || mounts.contains_key(&p.physical)
                {
                    return Err(
                        "8.5.3 complete original pair IDs/arcs/physical salvo capacity required"
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
                return Err("8.5.3 paired per-mount loaded+reserve salvos must exactly equal real aggregate ammunition".into());
            }
            let surviving = super::surviving_mounts(w, e.id, weapon, &hardware)?;
            super::verify_component_stock(w, e.id, weapon, &hardware, &surviving)?;
            s.units.entry(e.id).or_default().insert(
                weapon.clone(),
                Unit {
                    name: name.clone(),
                    class: e.class.clone(),
                    owner: e.side,
                    hardware,
                    model: model.clone(),
                    used_epoch: None,
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
        super::super::depth_charge::Action::ProjectorPairPattern { .. }
    )
}
fn request(actor: EntityId, a: &super::super::depth_charge::Action) -> Result<Request, String> {
    let super::super::depth_charge::Action::ProjectorPairPattern {
        contact,
        weapon,
        physicals,
        phase,
    } = a
    else {
        return Err("8.5.3 paired action required".into());
    };
    let mut ids = physicals.clone();
    ids.sort_unstable();
    Ok(Request {
        actor,
        contact: *contact,
        weapon: weapon.clone(),
        physicals: ids,
        phase: *phase,
    })
}
fn preflight(w: &World, s: &State, r: &Request) -> Result<Permit, String> {
    if !active(w) || w.over {
        return Err("8.5.3 actual active Full profile required".into());
    }
    let e = w
        .entity(r.actor)
        .filter(|e| e.alive)
        .ok_or("8.5.3 paired live original launcher absent")?;
    if crate::exercise::locked(w, e.side)
        || !super::super::super::unmanned2026::can_attack(w, r.actor)
        || !super::super::super::expanded_damage2026::platform_weapons_operational(w, r.actor)
        || !super::super::super::expanded_damage2026::weapon_operational(w, r.actor, &r.weapon)?
        || !super::super::super::damage_runtime::weapons_operational(e)
        || e.systems.fire_control <= 0.
    {
        return Err("8.5.3 actual paired-system crew/control/power unavailable".into());
    }
    super::super::super::nuclear2026::release_gate(w, r.actor, &r.weapon)?;
    let u = unit(s, r.actor, &r.weapon)?;
    if e.name != u.name || e.class != u.class || e.side != u.owner {
        return Err("8.5.3 original paired body/class/owner binding changed".into());
    }
    if r.physicals.is_empty()
        || r.physicals.len() > 2
        || r.physicals.windows(2).any(|p| p[0] == p[1])
        || r.physicals
            .iter()
            .any(|id| !u.hardware.mounts.contains_key(id))
    {
        return Err(
            "8.5.3 one or both distinct original members of the explicit pair required".into(),
        );
    }
    let epoch = w.tick.div_euclid(
        tables()["projector"]["advanced"]["tactical_seconds"]
            .as_i64()
            .unwrap(),
    );
    if r.phase != FirePhase::Planned || u.used_epoch == Some(epoch) {
        return Err("8.5.3 paired system has one actual Planned pattern per Tactical Turn, full or half; no separate half-mount reroll".into());
    }
    let h = &u.hardware;
    let a = e
        .weapons
        .get(h.mount_index)
        .filter(|a| a.weapon == r.weapon)
        .ok_or("8.5.3 original paired aggregate binding changed")?;
    let surviving = super::surviving_mounts(w, r.actor, &r.weapon, h)?;
    if super::surviving_stock(h, &surviving)? != a.ammo {
        return Err("8.5.3 paired aggregate/physical salvo stock conflict".into());
    }
    super::verify_component_stock(w, r.actor, &r.weapon, h, &surviving)?;
    let operational: BTreeSet<_> =
        super::super::super::expanded_damage2026::physical_weapon_mounts(
            w,
            r.actor,
            h.mount_index,
        )?
        .into_iter()
        .collect();
    let sea = super::sea_state(w)?;
    if f64::from(sea)
        >= tables()["projector"]["sea_state_exclusive_upper"]
            .as_f64()
            .unwrap()
    {
        return Err("8.5.3 no paired projector firing in sea state7+".into());
    }
    let c = w.sides[e.side.index()]
        .contacts
        .get(&r.contact)
        .filter(|c| !c.stale && !c.bearing_only)
        .ok_or("8.5.3 actual own located source contact required")?;
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
            if !attack_sonar::weapon_matches(w, r.actor, &r.weapon, &t.sensor) {
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
            let range = e.pos.range_to(aim);
            if solution.quality == Quality::NA || !range.is_finite() || range > u.model.range_nm {
                continue;
            }
            let bearing = e.pos.bearing_to(aim) - e.course_deg;
            let mut eligible = BTreeSet::new();
            for (id, m) in &h.mounts {
                if surviving.contains(id)
                    && operational.contains(id)
                    && m.loaded_patterns > 0
                    && super::super::super::aaw2026::arc_allows(&m.arc, bearing)?
                {
                    eligible.insert(*id);
                }
            }
            if eligible.is_empty() {
                continue;
            }
            candidates.push((eligible,solution.quality,aim,json!({"source":"own_native_same_receiver","sensor":t.sensor,"observer":r.actor,"at":report.at,"report":report,"motion":t.motion,"solution":solution,"dedicated_attack_sonar":attack_sonar::fcs_allowed(w,r.actor,c.about,&t.sensor,report)})));
        }
    }
    // Decide actual eligible membership before considering the player's chosen
    // subset. Any genuine full-pair solution prevents voluntary half firing.
    let maximum=candidates.iter().map(|p|p.0.len()).max().ok_or("8.5.3 no actual native measured FCS and available original pair member in source arc/range")?;
    candidates.retain(|p| p.0.len() == maximum);
    let sets: BTreeSet<_> = candidates.iter().map(|p| p.0.clone()).collect();
    if sets.len() != 1 {
        return Err("8.5.3 qualified measurements disagree about the sole eligible pair member; no invented sensor/ID priority".into());
    }
    let (eligible, quality, aim, evidence) = candidates.into_iter().max_by_key(|p| p.1).unwrap();
    let requested: BTreeSet<_> = r.physicals.iter().copied().collect();
    if requested != eligible {
        return Err(if eligible.len()==2 {"8.5.3 both original paired mounts can fire; both are required for the complete pattern"}else{"8.5.3 only the actual eligible pair member may fire; unavailable partner cannot be partially debited"}.into());
    }
    Ok(Permit {
        target: c.about,
        aim,
        quality,
        evidence,
        sea,
        epoch,
        one_of_required_pair: eligible.len() == 1,
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
        .paired
        .requests
        .push(r);
    Ok(())
}
fn reject(w: &mut World, r: &Request, reason: &str) {
    if let Some(side) = w.entity(r.actor).map(|e| e.side) {
        w.emit(
            EventKind::RulesEvent {
                domain: "asw_weapons2026".into(),
                rule: "8.5.3.paired_request_rejected".into(),
                side: Some(side),
                unit: Some(r.actor),
                contact: Some(r.contact),
                detail: json!({"weapon":r.weapon,"physicals":r.physicals,"reason":reason})
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
        .ok_or("8.5.3 paired pattern sequence exhausted")?;
    let id = format!(
        "8.5.3:paired:{}:{}:{}:{next}",
        r.actor.0, u.hardware.mount_index, permit.epoch
    );
    // Every unique original component and its full stock was checked before
    // this first debit. Distinct component writes cannot invalidate each other.
    for physical in &r.physicals {
        super::super::super::expanded_damage2026::spend_physical_weapon_stock(
            w,
            r.actor,
            &r.weapon,
            u.hardware.mount_index,
            *physical,
            1,
        )
        .expect("all paired component stocks validated before the transaction");
    }
    w.entity_mut(r.actor).unwrap().weapons[u.hardware.mount_index].ammo -= r.physicals.len() as u16;
    let own = s
        .units
        .get_mut(&r.actor)
        .unwrap()
        .get_mut(&r.weapon)
        .unwrap();
    for physical in &r.physicals {
        let m = own.hardware.mounts.get_mut(physical).unwrap();
        m.loaded_patterns -= 1;
        m.used_epoch = Some(permit.epoch);
    }
    own.used_epoch = Some(permit.epoch);
    s.next = next;
    let base = advanced::Pattern {
        id: id.clone(),
        actor: r.actor,
        owner: u.owner,
        target: permit.target,
        contact: r.contact,
        weapon: r.weapon.clone(),
        physical: r.physicals[0],
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
    };
    s.patterns.insert(
        id.clone(),
        Pattern {
            base,
            physicals: r.physicals.clone(),
            one_of_required_pair: permit.one_of_required_pair,
            result: None,
        },
    );
    super::super::trace(
        w,
        r.actor,
        "8.5.3.pattern_committed",
        json!({"attack_id":id,"weapon":r.weapon,"physicals":r.physicals,"stock_tokens":r.physicals.len(),"stock_unit":"physical_mount_salvo","logical_patterns":1,"one_of_required_pair":permit.one_of_required_pair,"source":"English2026 PDF99/8-19 source pair procedure; explicit authored physical salvo allocation"}),
    );
    let name = w.entity(r.actor).unwrap().name.clone();
    let weapon_name = w.db.weapons[&r.weapon].name.clone();
    w.emit(
        EventKind::WeaponsRelease {
            side: u.owner,
            shooter: r.actor,
            shooter_name: name,
            weapon: weapon_name,
            count: r.physicals.len() as u8,
        },
        Visibility::Side(u.owner),
    );
    Ok(())
}
pub(super) fn resolve(w: &mut World) {
    if !active(w) {
        return;
    }
    let Some(mut s) = w
        .rules
        .asw_weapons
        .as_mut()
        .map(|a| std::mem::take(&mut a.projectors.paired))
    else {
        return;
    };
    let mut groups: BTreeMap<(EntityId, String), BTreeMap<String, Request>> = BTreeMap::new();
    for r in std::mem::take(&mut s.requests) {
        groups
            .entry((r.actor, r.weapon.clone()))
            .or_default()
            .insert(serde_json::to_string(&r).unwrap(), r);
    }
    for choices in groups.values() {
        if choices.len() != 1 {
            for r in choices.values() {
                reject(w,r,"different simultaneous proposals for one logical paired system conflict; no partial physical/phase/network priority");
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
        // Exact same original one-D100/typed DP consumer as the accepted
        // singleton executor. Only the printed pair factor is an input here.
        let mut result = advanced::adjudicate(w, &p.base, p.one_of_required_pair);
        result["physicals"] = json!(p.physicals);
        result["one_of_required_pair"] = json!(p.one_of_required_pair);
        result["bombs_fired"] = json!(if p.one_of_required_pair {
            p.base.model.bombs / 2
        } else {
            p.base.model.bombs
        });
        result["stock_tokens"] = json!(p.physicals.len());
        result["stock_unit"] = json!("physical_mount_salvo");
        super::super::trace(w, p.base.actor, "8.5.3.pattern_result", result.clone());
        s.patterns.get_mut(&id).unwrap().result = Some(result);
    }
    w.rules.asw_weapons.as_mut().unwrap().projectors.paired = s;
}
pub(super) fn own_view(w: &World, actor: EntityId) -> Value {
    let Ok(s) = state(w) else {
        return Value::Null;
    };
    let Some(units) = s.units.get(&actor) else {
        return Value::Null;
    };
    let epoch = w.tick.div_euclid(
        tables()["projector"]["advanced"]["tactical_seconds"]
            .as_i64()
            .unwrap(),
    );
    let mut out = BTreeMap::new();
    for (weapon, u) in units {
        if !w
            .entity(actor)
            .is_some_and(|e| e.name == u.name && e.class == u.class && e.side == u.owner)
        {
            continue;
        }
        let surviving = super::surviving_mounts(w, actor, weapon, &u.hardware);
        let consistent = surviving.as_ref().is_ok_and(|set| {
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
            let destroyed=surviving.as_ref().is_ok_and(|set|!set.contains(&m.physical));
            json!({"physical":m.physical,"arc":m.arc,"loaded_patterns":if destroyed{0}else{m.loaded_patterns},"reserve_patterns":if destroyed{0}else{m.reserve_patterns},"operational":consistent&&operational.contains(&m.physical),"reload_pending":!destroyed&&m.loaded_patterns==0&&m.reserve_patterns>0})
        }).collect();
        out.insert(weapon,json!({"source":u.hardware.source,"fit_source":u.hardware.fit_source,"experimental_source":u.model.source,"model":"scenario_assumption","mount_index":u.hardware.mount_index,"stock_unit":"physical_mount_salvo","paired_mounts":[0,1],"bombs_per_complete_pattern":u.model.bombs,"pattern_used_this_turn":u.used_epoch==Some(epoch),"mounts":mounts,"attack_sonar":attack_sonar::own_view(w,actor,weapon),"reload_policy":null}));
    }
    json!(out)
}

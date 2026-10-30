//! §8.5.3 H-only RGB10 patterns. Missing reload and flight parameters remain
//! absent. RTS-PROJECTOR-PHASE-01 queues accepted Planned fire for the current
//! tick's common damage batch; it does not manufacture a ballistic projectile.
use super::{active, projector_thresholds, tables, Configuration, Quality};
use crate::db::{Guidance, WeaponKind};
use crate::types::PlatformKind;
use crate::{ContactId, EntityId, EventKind, Rng, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub mod advanced;
pub mod attack_sonar;
pub mod paired;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectorFit {
    pub source: String,
    pub model: String,
    pub units: BTreeMap<String, UnitFit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiment: Option<advanced::Model>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitFit {
    pub source: String,
    pub mount_index: usize,
    pub mounts: Vec<MountFit>,
    pub reload: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack_sensor: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MountFit {
    pub physical: usize,
    pub arc: String,
    pub loaded_patterns: u16,
    pub reserve_patterns: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MountState {
    pub physical: usize,
    pub arc: String,
    pub loaded_patterns: u16,
    pub reserve_patterns: u16,
    pub used_epoch: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitState {
    pub source: String,
    pub model: String,
    pub fit_source: String,
    pub mount_index: usize,
    pub mounts: BTreeMap<usize, MountState>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub units: BTreeMap<EntityId, BTreeMap<String, UnitState>>,
    pub patterns: BTreeMap<String, Pattern>,
    pub next_pattern: u64,
    #[serde(default, skip_serializing_if = "advanced::State::is_empty")]
    pub advanced: advanced::State,
    #[serde(default, skip_serializing_if = "attack_sonar::State::is_empty")]
    pub attack_sonars: attack_sonar::State,
    #[serde(default, skip_serializing_if = "paired::State::is_empty")]
    pub paired: paired::State,
}
impl State {
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
            && self.patterns.is_empty()
            && self.next_pattern == 0
            && self.advanced.is_empty()
            && self.attack_sonars.is_empty()
            && self.paired.is_empty()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    pub attack_id: String,
    pub actor: EntityId,
    pub side: Side,
    pub contact: ContactId,
    pub target: EntityId,
    pub weapon: String,
    pub model: String,
    pub mount_index: usize,
    pub physical: usize,
    pub origin: Vec2,
    pub course_deg: f64,
    pub aim: Vec2,
    pub range_nm: f64,
    pub quality: Quality,
    pub evidence: Value,
    pub tick: i64,
    pub epoch: i64,
    pub sea_state: u8,
    pub result: Option<PatternResult>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Thresholds {
    pub contact: Option<u32>,
    pub major: Option<u32>,
    pub minor: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternResult {
    pub raw_d100: Option<u8>,
    pub thresholds: Thresholds,
    pub severity: String,
    pub dp: Option<f64>,
    pub physical_speed_kts: Option<f64>,
    pub physical_depth_m: Option<f64>,
    pub physical_depth_band: Option<usize>,
    pub reason: Option<String>,
    pub damage_queued: bool,
}
/// No Deserialize/Serialize and private fields: a client cannot supply a
/// referee target, source solution, stock allocation, original die or result.
#[derive(Clone, Debug)]
pub struct ProjectorPermit {
    pattern: Pattern,
    inventory: Value,
}
#[derive(Clone, Debug, Deserialize)]
struct Model {
    weapon_ids: Vec<String>,
    fuzing: String,
    range_nm: f64,
    max_depth_band_index: usize,
    bombs: u8,
    contact_percent: Option<f64>,
    major_percent: f64,
    minor_percent: f64,
    damage_major: f64,
    damage_minor: f64,
    loaded_capacity_patterns: u16,
    attacks_per_tactical_turn: u8,
    physical_arcs: Vec<String>,
}
fn model(name: &str) -> Result<Model, String> {
    let value = tables()["projector"]["models"]
        .get(name)
        .ok_or("8.5.3 verified projector model absent")?;
    serde_json::from_value(value.clone()).map_err(|_| "8.5.3 invalid source model table".into())
}
fn runtime(w: &World) -> Result<&State, String> {
    Ok(&w
        .rules
        .asw_weapons
        .as_ref()
        .ok_or("8.5.3 runtime absent")?
        .projectors)
}
fn stock(mounts: &BTreeMap<usize, MountState>) -> Result<u16, String> {
    mounts.values().try_fold(0u16, |total, m| {
        total
            .checked_add(m.loaded_patterns)
            .and_then(|n| n.checked_add(m.reserve_patterns))
            .ok_or_else(|| "8.5.3 physical stock overflow".into())
    })
}
/// Physical destruction is distinct from power/director/crew availability.
/// In particular the operational-mount API must not erase stock just because
/// electrical failure temporarily prevents this powered projector firing.
fn surviving_mounts(
    w: &World,
    actor: EntityId,
    weapon: &str,
    u: &UnitState,
) -> Result<BTreeSet<usize>, String> {
    use super::super::expanded_damage2026::ComponentKind;
    let e = w.entity(actor).ok_or("8.5.3 physical owner absent")?;
    let aggregate = e
        .weapons
        .get(u.mount_index)
        .filter(|m| m.weapon == weapon)
        .ok_or("8.5.3 physical aggregate binding changed")?;
    let expanded = w.rules.expanded_damage.as_ref();
    let actual = expanded.and_then(|s| s.units.get(&actor));
    let fit = expanded.and_then(|s| {
        s.config
            .units
            .get(&e.name)
            .or_else(|| s.config.classes.get(&e.class))
    });
    let mut live = BTreeSet::new();
    for p in u.mounts.keys() {
        let native = format!("native.mount:{}:{p}", u.mount_index);
        let explicit: Vec<_> = fit
            .into_iter()
            .flat_map(|f| f.components.iter())
            .filter(|(_, c)| {
                c.kind == ComponentKind::WeaponMount
                    && c.binding.as_deref() == Some(weapon)
                    && c.physical == Some(*p)
                    && c.mount_index.is_none_or(|i| i == u.mount_index)
            })
            .collect();
        if explicit.len() > 1 {
            return Err("8.5.3 ambiguous physical casualty component".into());
        }
        let key = explicit
            .first()
            .map(|(key, _)| key.as_str())
            .unwrap_or(&native);
        let destroyed = actual.is_some_and(|s| s.disabled.contains(key))
            || e.ext
                .get(&format!("damage.mount.{}.{p}", u.mount_index))
                .is_some_and(|v| *v > 0.);
        if !destroyed {
            live.insert(*p);
        }
    }
    if live.len() != usize::from(aggregate.mounts) {
        return Err("8.5.3 aggregate physical count lacks exact surviving-mount identity".into());
    }
    Ok(live)
}
fn surviving_stock(u: &UnitState, live: &BTreeSet<usize>) -> Result<u16, String> {
    let mounts = u
        .mounts
        .iter()
        .filter(|(p, _)| live.contains(p))
        .map(|(p, m)| (*p, m.clone()))
        .collect();
    stock(&mounts)
}
fn verify_component_stock(
    w: &World,
    actor: EntityId,
    weapon: &str,
    u: &UnitState,
    live: &BTreeSet<usize>,
) -> Result<(), String> {
    for p in live {
        let m = &u.mounts[p];
        let expected = m
            .loaded_patterns
            .checked_add(m.reserve_patterns)
            .ok_or("8.5.3 selected stock overflow")?;
        if super::super::expanded_damage2026::physical_weapon_stock(
            w,
            actor,
            weapon,
            u.mount_index,
            *p,
        )?
        .is_some_and(|n| n != expected)
        {
            return Err("8.5.3 exact physical component/pattern ammunition conflict".into());
        }
    }
    Ok(())
}
/// Pure initializer. The parent installs this state only after all ASW fits
/// validate, so neither invalid late rows nor duplicate names publish a prefix.
pub(super) fn initialize(w: &World, config: &Configuration) -> Result<State, String> {
    let mut state = State::default();
    state.attack_sonars = attack_sonar::initialize(w, config)?;
    state.advanced = advanced::initialize(w, config)?;
    state.paired = paired::initialize(w, config)?;
    for (weapon, fit) in config
        .projectors
        .iter()
        .filter(|(_, f)| f.experiment.is_none())
    {
        let source = model(&fit.model)?;
        let def =
            w.db.weapons
                .get(weapon)
                .ok_or("8.5.3 weapon definition absent")?;
        if fit.source.trim().is_empty()
            || fit.units.is_empty()
            || source.fuzing != "H"
            || source.contact_percent.is_some()
            || source.attacks_per_tactical_turn != 1
            || !source.weapon_ids.contains(weapon)
            || def.kind != WeaponKind::AswRocket
            || def.guidance != Guidance::AswBallistic
            || config.standoffs.contains_key(weapon)
        {
            return Err(
                "8.5.3 exact H-only projector source/weapon fit required; not standoff".into(),
            );
        }
        for (name, unit) in &fit.units {
            let matching: Vec<_> = w.entities.values().filter(|e| e.name == *name).collect();
            if matching.len() != 1 {
                return Err("8.5.3 projector unit name must identify exactly one entity".into());
            }
            let e = matching[0];
            let m = e
                .weapons
                .get(unit.mount_index)
                .ok_or("8.5.3 aggregate mount absent")?;
            if !e.alive
                || e.kind != PlatformKind::Ship
                || unit.source.trim().is_empty()
                || unit.reload.is_some()
                || m.weapon != *weapon
                || m.mounts == 0
                || e.weapons.iter().filter(|m| m.weapon == *weapon).count() != 1
                || unit.mounts.len() != usize::from(m.mounts)
            {
                return Err("8.5.3 unique live source ship/mount, complete allocation and unresolved reload required".into());
            }
            let mut mounts = BTreeMap::new();
            for p in &unit.mounts {
                if p.physical >= usize::from(m.mounts)
                    || p.loaded_patterns > source.loaded_capacity_patterns
                    || source.physical_arcs.get(p.physical) != Some(&p.arc)
                    || mounts.contains_key(&p.physical)
                {
                    return Err("8.5.3 duplicate/missing physical mount, unprinted arc or invalid loaded pattern stock".into());
                }
                super::super::aaw2026::arc_allows(&p.arc, 90.)?;
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
            if stock(&mounts)? != m.ammo {
                return Err(
                    "8.5.3 loaded plus reserve patterns must equal actual aggregate inventory"
                        .into(),
                );
            }
            let unit_state = UnitState {
                source: unit.source.clone(),
                model: fit.model.clone(),
                fit_source: fit.source.clone(),
                mount_index: unit.mount_index,
                mounts,
            };
            let live = surviving_mounts(w, e.id, weapon, &unit_state)?;
            verify_component_stock(w, e.id, weapon, &unit_state, &live)?;
            state
                .units
                .entry(e.id)
                .or_default()
                .insert(weapon.clone(), unit_state);
        }
    }
    Ok(state)
}
pub fn is_projector_configured(w: &World, actor: EntityId, weapon: &str) -> bool {
    active(w)
        && runtime(w)
            .ok()
            .and_then(|s| s.units.get(&actor))
            .is_some_and(|u| u.contains_key(weapon))
        || advanced::configured(w, actor, weapon)
        || paired::configured(w, actor, weapon)
}
fn sea_state(w: &World) -> Result<u8, String> {
    w.context
        .radar_environment
        .as_ref()
        .map(|e| e.sea_state)
        .ok_or_else(|| "8.5.3 explicit source sea state required".into())
}
/// Read-only preflight. Enemy physics/life are deliberately not consulted.
pub fn validate_projector(
    w: &World,
    actor: EntityId,
    contact: ContactId,
    weapon: &str,
    rounds: u32,
) -> Result<ProjectorPermit, String> {
    if !active(w) || rounds != 1 {
        return Err("8.5.3 Full profile and one complete pattern per Fire required".into());
    }
    let e = w
        .entity(actor)
        .filter(|e| e.alive)
        .ok_or("8.5.3 own launcher unavailable")?;
    if !super::super::damage_runtime::weapons_operational(e) || e.systems.fire_control <= 0. {
        return Err("8.5.3 launcher control/power/crew unavailable".into());
    }
    let u = runtime(w)?
        .units
        .get(&actor)
        .and_then(|u| u.get(weapon))
        .ok_or("8.5.3 physical projector source fit absent")?;
    let source = model(&u.model)?;
    let m = e
        .weapons
        .get(u.mount_index)
        .filter(|m| m.weapon == weapon)
        .ok_or("8.5.3 aggregate source mount changed")?;
    let surviving = surviving_mounts(w, actor, weapon, u)?;
    if surviving_stock(u, &surviving)? != m.ammo {
        return Err(
            "8.5.3 physical/aggregate inventory inconsistent; no redistribution inferred".into(),
        );
    }
    verify_component_stock(w, actor, weapon, u, &surviving)?;
    let sea = sea_state(w)?;
    if sea
        >= tables()["projector"]["sea_state_exclusive_upper"]
            .as_u64()
            .unwrap() as u8
    {
        return Err("8.5.3 projectors cannot fire in sea state seven or higher".into());
    }
    let c = w.sides[e.side.index()]
        .contacts
        .get(&contact)
        .filter(|c| !c.stale && !c.bearing_only)
        .ok_or("8.5.3 own current located CID required")?;
    if super::super::detection2026::known_category(w, e.side, c.about) != Some(PlatformKind::Sub) {
        return Err("8.5.3 own contact must actually be classified submarine".into());
    }
    let live: BTreeSet<_> =
        super::super::expanded_damage2026::physical_weapon_mounts(w, actor, u.mount_index)?
            .into_iter()
            .collect();
    let epoch = w.tick.div_euclid(180);
    let mut candidates = Vec::new();
    if let Some(aaw) = &w.rules.aaw {
        for track in aaw
            .firecontrol
            .tracks
            .values()
            .filter(|t| t.observer == actor && t.target == c.about)
        {
            let Some(report) = &track.report else {
                continue;
            };
            if !attack_sonar::weapon_matches(w, actor, weapon, &track.sensor) {
                continue;
            }
            let Some(aim) = report.measured_position else {
                continue;
            };
            let Ok(solution) = super::super::firecontrol2026::own_projector_solution(
                w,
                actor,
                c.about,
                &track.sensor,
            ) else {
                continue;
            };
            if solution.quality == Quality::NA {
                continue;
            }
            let range = e.pos.range_to(aim);
            if !range.is_finite() || range > source.range_nm {
                continue;
            }
            let bearing = e.pos.bearing_to(aim) - e.course_deg;
            let eligible: Vec<_> = u
                .mounts
                .values()
                .filter(|p| {
                    live.contains(&p.physical)
                        && p.loaded_patterns > 0
                        && p.used_epoch != Some(epoch)
                        && super::super::aaw2026::arc_allows(&p.arc, bearing).unwrap_or(false)
                })
                .collect();
            // No unprinted physical choice policy or ID ordering is invented.
            if eligible.len() != 1 {
                continue;
            }
            candidates.push((
                solution.quality,
                aim,
                range,
                eligible[0].physical,
                json!({"source":"own_native_same_receiver","sensor":track.sensor,"observer":actor,
                    "at":report.at,"report":report,"motion":track.motion,"solution":solution,
                    "projector_without_torpedo_seeker":true,
                    "dedicated_attack_sonar":attack_sonar::fcs_allowed(w,actor,c.about,&track.sensor,report)}),
            ));
        }
    }
    let (quality, aim, range_nm, physical, evidence) = candidates.into_iter().max_by_key(|c| c.0)
        .ok_or("8.5.3 no qualified Gen4+ active measured FCS and unique loaded physical mount in source range/arc")?;
    Ok(ProjectorPermit {
        pattern: Pattern {
            attack_id: String::new(),
            actor,
            side: e.side,
            contact,
            target: c.about,
            weapon: weapon.into(),
            model: u.model.clone(),
            mount_index: u.mount_index,
            physical,
            origin: e.pos,
            course_deg: e.course_deg,
            aim,
            range_nm,
            quality,
            evidence,
            tick: w.tick,
            epoch,
            sea_state: sea,
            result: None,
        },
        // A simultaneous valid pattern on the other physical mount must not
        // invalidate this mount's permit. Current aggregate conservation is
        // rechecked separately; freeze only the selected physical allocation.
        inventory: json!({"selected":u.mounts.get(&physical),"component_stock":super::super::expanded_damage2026::physical_weapon_stock(w,actor,weapon,u.mount_index,physical)?}),
    })
}
/// Revalidate the exact source snapshot before the first mutation. A cloned
/// permit cannot pass again after its physical pattern has been spent.
pub fn commit_projector(w: &mut World, permit: ProjectorPermit) -> Result<String, String> {
    let p = &permit.pattern;
    let fresh = validate_projector(w, p.actor, p.contact, &p.weapon, 1)?;
    if serde_json::to_value(&fresh.pattern).unwrap() != serde_json::to_value(p).unwrap()
        || fresh.inventory != permit.inventory
    {
        return Err("8.5.3 stale or changed source permit".into());
    }
    let next = runtime(w)?
        .next_pattern
        .checked_add(1)
        .ok_or("8.5.3 pattern identity overflow")?;
    let attack_id = format!(
        "8.5.3:{}:{}:{}:{}:{}",
        p.actor.0, p.mount_index, p.physical, p.epoch, next
    );
    if runtime(w)?.patterns.contains_key(&attack_id) {
        return Err("8.5.3 duplicate pattern identity".into());
    }
    let mut pattern = permit.pattern;
    pattern.attack_id = attack_id.clone();
    // All source/permit/aggregate checks have passed. The exact helper either
    // charges this physical component or is a no-op when no such fit exists.
    super::super::expanded_damage2026::spend_physical_weapon_stock(
        w,
        pattern.actor,
        &pattern.weapon,
        pattern.mount_index,
        pattern.physical,
        1,
    )?;
    w.entity_mut(pattern.actor).unwrap().weapons[pattern.mount_index].ammo -= 1;
    let s = &mut w.rules.asw_weapons.as_mut().unwrap().projectors;
    let m = s
        .units
        .get_mut(&pattern.actor)
        .unwrap()
        .get_mut(&pattern.weapon)
        .unwrap()
        .mounts
        .get_mut(&pattern.physical)
        .unwrap();
    m.loaded_patterns -= 1;
    m.used_epoch = Some(pattern.epoch);
    s.next_pattern = next;
    s.patterns.insert(attack_id.clone(), pattern.clone());
    // The dedicated projector allocation is authoritative. The generic helper
    // chooses a component by ID, which could spend the opposite physical mount.
    super::trace(
        w,
        pattern.actor,
        "8.5.3.pattern_committed",
        json!({"attack_id":attack_id,
        "weapon":pattern.weapon,"model":pattern.model,"aggregate_mount_index":pattern.mount_index,
        "physical":pattern.physical,"bombs":model(&pattern.model)?.bombs,"stock_tokens":1,
        "stock_unit":"complete_pattern","phase_policy":"RTS-PROJECTOR-PHASE-01",
        "epoch":pattern.epoch,"aim":pattern.aim,"source_fcs":pattern.evidence}),
    );
    let weapon_name = w.db.weapons.get(&pattern.weapon).unwrap().name.clone();
    let shooter_name = w.entity(pattern.actor).unwrap().name.clone();
    w.emit(
        EventKind::WeaponsRelease {
            side: pattern.side,
            shooter: pattern.actor,
            shooter_name,
            weapon: weapon_name,
            count: 1,
        },
        Visibility::Side(pattern.side),
    );
    Ok(attack_id)
}
fn adjudicate(w: &World, p: &Pattern) -> PatternResult {
    let mut out = PatternResult {
        raw_d100: None,
        thresholds: Thresholds {
            contact: None,
            major: Some(0),
            minor: Some(0),
        },
        severity: "miss".into(),
        dp: None,
        physical_speed_kts: None,
        physical_depth_m: None,
        physical_depth_band: None,
        reason: None,
        damage_queued: false,
    };
    if p.quality == Quality::Poor {
        out.reason = Some("8.5.3 Poor FCS automatically misses; no pattern die".into());
        return out;
    }
    let mut rng = w.rng.derive(Rng::hash_tag(
        p.tick,
        p.actor.0,
        &format!("{}:pattern_d100", p.attack_id),
    ));
    out.raw_d100 = Some(1 + (rng.next_f64() * 100.).floor() as u8);
    let Some(target) = w.entity(p.target) else {
        out.severity = "pending".into();
        out.reason = Some("8.5.3 referee target physics unavailable after expenditure".into());
        return out;
    };
    out.physical_speed_kts = Some(target.speed_kts.abs());
    out.physical_depth_m = Some(target.depth_m);
    if !target.alive || target.kind != PlatformKind::Sub {
        out.reason = Some("8.5.3 no live submarine at physical adjudication".into());
        return out;
    }
    if target.depth_m == 0. {
        out.severity = "pending".into();
        out.thresholds.major = None;
        out.thresholds.minor = None;
        out.reason = Some(
            "8.5.3 source matrix has no Surface H-only effect; spent pattern retained pending"
                .into(),
        );
        return out;
    }
    let Ok(band) = super::super::detection2026::depth_band(target.depth_m) else {
        out.severity = "pending".into();
        out.reason = Some("8.5.3 physical target depth unavailable".into());
        return out;
    };
    out.physical_depth_band = Some(band);
    let source = model(&p.model).expect("initialized source projector model");
    if band > source.max_depth_band_index {
        out.reason =
            Some("8.5.3 target below source Int IV maximum; spent pattern has no effect".into());
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
    let values = match projector_thresholds(
        [None, Some(source.major_percent), Some(source.minor_percent)],
        p.quality,
        target.speed_kts.abs(),
        col,
        p.sea_state,
        false,
    ) {
        Ok(v) => v,
        Err(reason) => {
            if !reason.contains("attack not possible") {
                out.severity = "pending".into();
            }
            out.reason = Some(reason);
            return out;
        }
    };
    out.thresholds.major = Some(values[1]);
    out.thresholds.minor = Some(values[2]);
    let d = u32::from(out.raw_d100.unwrap());
    if d <= values[1] {
        out.severity = "major".into();
        out.dp = Some(source.damage_major);
    } else if d <= values[2] {
        out.severity = "minor".into();
        out.dp = Some(source.damage_minor);
    }
    out
}
/// Idempotent same-tick resolver. The original result is stored before damage
/// batch closure; save/restore never rolls the pattern again or spends stock.
pub fn resolve_projector_patterns(w: &mut World) {
    if !active(w) {
        return;
    }
    let patterns: Vec<_> = runtime(w)
        .map(|s| {
            s.patterns
                .values()
                .filter(|p| p.result.is_none() && p.tick <= w.tick)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    for p in patterns {
        let mut result = adjudicate(w, &p);
        if let Some(dp) = result.dp {
            match super::super::damage_runtime::queue_projector(
                w,
                p.actor,
                p.target,
                &p.weapon,
                dp,
                result.severity == "major",
                &p.attack_id,
            ) {
                Ok(()) => result.damage_queued = true,
                Err(reason) => {
                    result.reason = Some(format!("8.5.3 damage consumer pending: {reason}"))
                }
            }
        }
        w.rules
            .asw_weapons
            .as_mut()
            .unwrap()
            .projectors
            .patterns
            .get_mut(&p.attack_id)
            .unwrap()
            .result = Some(result.clone());
        super::trace(
            w,
            p.actor,
            "8.5.3.pattern_result",
            json!({
                "attack_id":p.attack_id,"weapon":p.weapon,"model":p.model,
                "physical":p.physical,"aggregate_mount_index":p.mount_index,"epoch":p.epoch,
                "bombs":model(&p.model).unwrap().bombs,"raw_d100":result.raw_d100,
                "thresholds":result.thresholds,"severity":result.severity,"dp":result.dp,
                "quality":p.quality,"physical_speed_kts":result.physical_speed_kts,
                "physical_depth_m":result.physical_depth_m,"physical_depth_band":result.physical_depth_band,
                "reason":result.reason,"damage_queued":result.damage_queued,
                "source_fcs":p.evidence,"phase_policy":"RTS-PROJECTOR-PHASE-01"
            }),
        );
    }
    advanced::resolve(w);
    paired::resolve(w);
}
/// Only own fitting, ammunition and known physical equipment status. No target
/// truth, original dice, probabilities, hit/miss or damage-result ledger.
pub fn projector_own_view(w: &World, actor: EntityId) -> Value {
    let mut extra = advanced::own_view(w, actor);
    let pair_view = paired::own_view(w, actor);
    if let Some(pair_view) = pair_view.as_object() {
        if !extra.is_object() {
            extra = json!({});
        }
        extra.as_object_mut().unwrap().extend(pair_view.clone());
    }
    let Some(units) = runtime(w).ok().and_then(|s| s.units.get(&actor)) else {
        return extra;
    };
    let mut view = BTreeMap::new();
    for (weapon, u) in units {
        let surviving = surviving_mounts(w, actor, weapon, u).ok();
        let consistent = surviving.as_ref().is_some_and(|alive| {
            surviving_stock(u, alive).ok()
                == w.entity(actor)
                    .and_then(|e| e.weapons.get(u.mount_index))
                    .map(|m| m.ammo)
                && verify_component_stock(w, actor, weapon, u, alive).is_ok()
        });
        let live =
            super::super::expanded_damage2026::physical_weapon_mounts(w, actor, u.mount_index)
                .unwrap_or_default();
        let mounts: Vec<_> = u
            .mounts
            .values()
            .map(|m| {
                let destroyed = surviving.as_ref().is_some_and(|s| !s.contains(&m.physical));
                json!({
                    "physical":m.physical,"arc":m.arc,"loaded_patterns":if destroyed {0}else{m.loaded_patterns},
                    "reserve_patterns":if destroyed {0}else{m.reserve_patterns},"used_epoch":m.used_epoch,
                    "status":if destroyed {"destroyed"}else if !consistent {"source_pending"}else if live.contains(&m.physical){"operational"}else{"unavailable"},
                    "operational":live.contains(&m.physical),
                    "reload_pending":!destroyed && m.loaded_patterns==0 && m.reserve_patterns>0
                })
            })
            .collect();
        view.insert(weapon, json!({"source":u.source,"fit_source":u.fit_source,"model":u.model,
            "mount_index":u.mount_index,"stock_unit":"complete_pattern","bombs_per_pattern":model(&u.model).unwrap().bombs,
            "reload_policy":Value::Null,"mounts":mounts,"attack_sonar":attack_sonar::own_view(w,actor,weapon)}));
    }
    let mut view = json!(view);
    if let Some(extra) = extra.as_object() {
        view.as_object_mut().unwrap().extend(extra.clone());
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;

    // These are strict configuration and pure source boundaries. The separate
    // native integration candidates exercise actual reports/Fire/damage.
    fn configured() -> (World, Configuration) {
        let mut db = crate::Database::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../database/consolidated.json"
        )))
        .unwrap();
        let original = db.platforms["cn_956em"]
            .weapons
            .iter()
            .find(|m| m.weapon == "cn_rbu1000")
            .unwrap()
            .clone();
        let index = db.platforms["us_ford"].weapons.len();
        db.platforms
            .get_mut("us_ford")
            .unwrap()
            .weapons
            .push(original);
        let scenario: crate::Scenario = serde_json::from_value(json!({
            "id":"projector-config-unit","name":"scenario_assumption: original Ford with original956EM experimental mount",
            "seed":1,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"}},
            "units":[{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false}]
        })).unwrap();
        let w = crate::load_scenario(scenario, db).unwrap();
        let config = serde_json::from_value(json!({"source":"scenario_assumption: source RGB10 and explicit pattern stock",
            "standoffs":{},"projectors":{"cn_rbu1000":{"source":"China2025 PDF75 E3 and PDF16 source P/S mount","model":"rbu1000_rgb10",
            "units":{"LAB":{"source":"scenario_assumption: exact experimental initial stock, no reload claim","mount_index":index,"reload":null,
                "mounts":[{"physical":0,"arc":"P","loaded_patterns":1,"reserve_patterns":3},{"physical":1,"arc":"S","loaded_patterns":1,"reserve_patterns":3}]}}}}})).unwrap();
        (w, config)
    }
    fn native_two_targets() -> (World, EntityId, [ContactId; 2]) {
        let (base, config) = configured();
        let mut db = base.db;
        const ACTIVE: &str = "sonar_a_projector_unit_native";
        const PASSIVE: &str = "projector_unit_native_passive";
        for (id, kind) in [
            (ACTIVE, crate::db::SensorKind::SonarActive),
            (PASSIVE, crate::db::SensorKind::SonarPassive),
        ] {
            let mut sensor = db.sensors.values().next().unwrap().clone();
            sensor.id = id.into();
            sensor.name = format!("scenario_assumption:{id}");
            sensor.kind = kind;
            sensor.air_range_nm = 0.;
            sensor.surface_range_nm = 0.;
            sensor.missile_range_nm = 0.;
            sensor.sub_range_nm = 16.;
            db.sensors.insert(id.into(), sensor);
        }
        db.platforms.get_mut("us_ford").unwrap().sensors = vec![ACTIVE.into(), PASSIVE.into()];
        db.platforms.get_mut("cn_093").unwrap().sensors.clear();
        db.validate().unwrap();
        let receiver = |active| json!({"type":"sonar","source":"scenario_assumption: independent laboratory Gen5 LMF16nm Bow; no real Ford sonar claim","generation":5,"passive_range_nm":if active{None}else{Some(16.)},"active_range_nm":if active{Some(16.)}else{None},"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false});
        let index = config.projectors["cn_rbu1000"].units["LAB"].mount_index;
        let component = |physical| json!({"source":"scenario_assumption: exact borrowed956EM physical allocation4 full patterns","kind":"weapon_mount","binding":"cn_rbu1000","physical":physical,"mount_index":index,"ammunition":4});
        let sc:crate::Scenario=serde_json::from_value(json!({"id":"projector-native-unit","name":"scenario_assumption: original Ford CS6/DP plus original956EM RBU stock and two experimental sonar receivers","seed":20261025,"duration_s":10000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":{
            "rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},"asw_weapons_2026":config,
            "expanded_damage_2026":{"source":"scenario_assumption: exact physical pattern inventory","units":{"LAB":{"source":"scenario_assumption: two original physical mounts with explicit pattern allocation","components":{"port":component(0),"starboard":component(1)}}}},
            "detection_2026":{"bindings":{format!("us_ford:{ACTIVE}"):receiver(true),format!("us_ford:{PASSIVE}"):receiver(false)},"environment":{"source":"scenario_assumption: explicit open water laboratory","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},"targets":{"cn_093":{"source":"scenario_assumption: explicit acoustic signature, original DP","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
            "units":[{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"course_deg":0,"speed_kts":0,"sonar_active":true,"fire_on":false},{"side":"Red","class":"cn_093","name":"EAST","pos":[0.3,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false},{"side":"Red","class":"cn_093","name":"WEST","pos":[-0.3,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}]
        })).unwrap();
        let mut w = crate::load_scenario(sc, db).unwrap();
        let actor = w.entities.values().find(|e| e.name == "LAB").unwrap().id;
        let targets =
            ["EAST", "WEST"].map(|name| w.entities.values().find(|e| e.name == name).unwrap().id);
        let mut contacts = None;
        while w.tick < 6000 {
            let found: Vec<_> = targets
                .iter()
                .filter_map(|target| {
                    if super::super::super::detection2026::known_category(&w, Side::Blue, *target)
                        != Some(PlatformKind::Sub)
                        || !super::super::super::firecontrol2026::own_projector_solution(
                            &w, actor, *target, ACTIVE,
                        )
                        .is_ok_and(|s| s.quality == Quality::Good)
                    {
                        return None;
                    }
                    w.sides[0]
                        .contacts
                        .values()
                        .find(|c| c.about == *target && !c.stale && !c.bearing_only)
                        .map(|c| c.id)
                })
                .collect();
            if found.len() == 2 {
                contacts = Some([found[0], found[1]]);
                break;
            }
            assert!(!w.over);
            crate::sim::tick(&mut w, &[]);
        }
        (
            w,
            actor,
            contacts.expect("two actual located/classified native same-receiver Good solutions"),
        )
    }
    #[test]
    fn native_projector_permit_validation_is_read_only_and_clone_cannot_commit_twice() {
        let (mut w, actor, cids) = native_two_targets();
        let before = serde_json::to_vec(&w).unwrap();
        let permit = validate_projector(&w, actor, cids[0], "cn_rbu1000", 1).unwrap();
        assert_eq!(serde_json::to_vec(&w).unwrap(), before);
        let clone = permit.clone();
        commit_projector(&mut w, permit).unwrap();
        let committed = serde_json::to_vec(&w).unwrap();
        assert!(commit_projector(&mut w, clone).is_err());
        assert_eq!(serde_json::to_vec(&w).unwrap(), committed);
    }
    #[test]
    fn native_opposite_physical_permits_preformed_in_one_tick_keep_exact_component_stock() {
        let (mut w, actor, cids) = native_two_targets();
        let east = validate_projector(&w, actor, cids[0], "cn_rbu1000", 1).unwrap();
        let west = validate_projector(&w, actor, cids[1], "cn_rbu1000", 1).unwrap();
        assert_ne!(east.pattern.physical, west.pattern.physical);
        let index = east.pattern.mount_index;
        commit_projector(&mut w, east).unwrap();
        commit_projector(&mut w, west).unwrap();
        assert_eq!(w.entity(actor).unwrap().weapons[index].ammo, 6);
        for physical in [0, 1] {
            assert_eq!(
                super::super::super::expanded_damage2026::physical_weapon_stock(
                    &w,
                    actor,
                    "cn_rbu1000",
                    index,
                    physical
                )
                .unwrap(),
                Some(3)
            );
        }
        assert_eq!(runtime(&w).unwrap().patterns.len(), 2);
        super::super::super::damage_runtime::begin(&mut w);
        resolve_projector_patterns(&mut w);
        super::super::super::damage_runtime::finish(&mut w);
        assert!(runtime(&w)
            .unwrap()
            .patterns
            .values()
            .all(|p| p.result.as_ref().is_some_and(|r| r.raw_d100.is_some())));
    }
    #[test]
    fn native_actual_surface_arrival_spends_a_pattern_but_h_effect_remains_pending() {
        let (initial, actor, cids) = native_two_targets();
        let target = initial.sides[0].contacts[&cids[0]].about;
        let mut selected = None;
        let mut attempts = Vec::new();
        // A 90%-capped acoustic trial need not succeed in the exact epoch when
        // this ordinary surfacing finishes. Select an actual timing window,
        // retaining every rejected gate reason; do not fabricate a report or
        // broaden the source preflight to make one arbitrary start time pass.
        for delay_turns in 0..=6 {
            let mut trial = initial.clone();
            while trial.tick < initial.tick + delay_turns * 180 {
                crate::sim::tick(&mut trial, &[]);
            }
            crate::sim::tick(
                &mut trial,
                &[(
                    Side::Red,
                    crate::Command::SetDepth {
                        unit: target,
                        depth_m: 0.,
                    },
                )],
            );
            let until = trial.tick + 200;
            while trial.tick < until {
                assert!(trial.entity(target).unwrap().depth_m > 0.);
                // Fire executes before movement. Only the clone forecasts the
                // next ordinary movement; the chosen original is never edited.
                let mut next = trial.clone();
                crate::sim::tick(&mut next, &[]);
                if next.entity(target).unwrap().depth_m == 0. {
                    let current_cid = trial.sides[0]
                        .contacts
                        .values()
                        .find(|c| c.about == target && !c.stale && !c.bearing_only)
                        .map(|c| c.id);
                    let gate = current_cid
                        .ok_or_else(|| "no current own located contact".to_string())
                        .and_then(|cid| {
                            validate_projector(&trial, actor, cid, "cn_rbu1000", 1)
                                .map(|permit| (cid, permit.pattern.quality))
                        });
                    let native_fcs = super::super::super::firecontrol2026::own_projector_solution(
                        &trial,
                        actor,
                        target,
                        "sonar_a_projector_unit_native",
                    );
                    attempts.push(format!(
                        "delay_turns={delay_turns} tick={} depth={} own_category={:?} gate={gate:?} native_fcs={native_fcs:?}",
                        trial.tick,
                        trial.entity(target).unwrap().depth_m,
                        super::super::super::detection2026::known_category(&trial, Side::Blue, target),
                    ));
                    if let Ok((cid, Quality::Good)) = gate {
                        selected = Some((trial, cid));
                    }
                    break;
                }
                crate::sim::tick(&mut trial, &[]);
            }
            if selected.is_some() {
                break;
            }
        }
        eprintln!("actual Surface timing preflights: {attempts:#?}");
        let (mut w, cid) = selected.unwrap_or_else(|| {
            panic!("no real pre-movement Good window for actual Surface arrival: {attempts:#?}")
        });
        assert!(w.entity(target).unwrap().depth_m > 0.);
        let hp = w.entity(target).unwrap().hp;
        let index = runtime(&w).unwrap().units[&actor]["cn_rbu1000"].mount_index;
        let ammo = w.entity(actor).unwrap().weapons[index].ammo;
        crate::sim::tick(
            &mut w,
            &[(
                Side::Blue,
                crate::Command::Fire {
                    unit: actor,
                    contact: cid,
                    weapon: "cn_rbu1000".into(),
                    rounds: 1,
                },
            )],
        );
        assert_eq!(
            w.entity(target).unwrap().depth_m,
            0.,
            "actual ordinary movement reaches Surface in the firing tick; no truth write"
        );
        assert_eq!(
            w.entity(actor).unwrap().weapons[index].ammo,
            ammo - 1,
            "actual pre-movement native Sub evidence permits expenditure before same-tick Surface arrival"
        );
        let result = runtime(&w)
            .unwrap()
            .patterns
            .values()
            .next()
            .unwrap()
            .result
            .as_ref()
            .unwrap();
        assert_eq!(result.severity, "pending");
        assert!(result.dp.is_none());
        assert!(!result.damage_queued);
        assert_eq!(w.entity(target).unwrap().hp, hp);
        assert!(!w
            .events
            .iter()
            .any(|event| matches!(&event.kind, EventKind::Impact { unit, .. } if *unit == target)));
    }
    #[test]
    fn projector_initialization_rejects_phantom_or_misallocated_loaded_stock() {
        let (w, good) = configured();
        assert!(initialize(&w, &good).is_ok());
        for (loaded, reserve) in [(2, 2), (1, 4), (0, 0)] {
            let mut cfg = good.clone();
            let mount = &mut cfg
                .projectors
                .get_mut("cn_rbu1000")
                .unwrap()
                .units
                .get_mut("LAB")
                .unwrap()
                .mounts[0];
            mount.loaded_patterns = loaded;
            mount.reserve_patterns = reserve;
            assert!(initialize(&w, &cfg).is_err());
        }
    }
    #[test]
    fn projector_initialization_requires_unique_physical_and_aggregate_identity() {
        let (mut w, good) = configured();
        let mut duplicate = good.clone();
        let mounts = &mut duplicate
            .projectors
            .get_mut("cn_rbu1000")
            .unwrap()
            .units
            .get_mut("LAB")
            .unwrap()
            .mounts;
        mounts[1].physical = 0;
        mounts[1].arc = "P".into();
        assert!(initialize(&w, &duplicate).is_err());
        let actor = w.entities.values().next().unwrap().id;
        let index = good.projectors["cn_rbu1000"].units["LAB"].mount_index;
        let mount = w.entity(actor).unwrap().weapons[index].clone();
        w.entity_mut(actor).unwrap().weapons.push(mount);
        assert!(initialize(&w, &good).is_err());
    }
    #[test]
    fn projector_invalid_late_fit_cannot_publish_partial_runtime() {
        let (mut w, good) = configured();
        w.context.asw_weapons_2026 = Some(good.clone());
        super::super::initialize(&mut w).unwrap();
        let before = serde_json::to_value(&w.rules.asw_weapons).unwrap();
        let mut bad = good.clone();
        let fit = bad.projectors["cn_rbu1000"].clone();
        bad.projectors.insert("zz_missing_real_weapon".into(), fit);
        w.context.asw_weapons_2026 = Some(bad);
        assert!(super::super::initialize(&mut w).is_err());
        assert_eq!(serde_json::to_value(&w.rules.asw_weapons).unwrap(), before);
        let mut unknown = good;
        unknown.projectors.get_mut("cn_rbu1000").unwrap().model = "unknown_projector".into();
        assert!(initialize(&w, &unknown).is_err());
    }
    #[test]
    fn projector_source_thresholds_keep_absolute_minor_and_final_rounding() {
        assert_eq!(
            projector_thresholds([None, Some(11.), Some(20.)], Quality::Good, 0., 0, 0, false)
                .unwrap(),
            [0, 11, 20]
        );
        assert_eq!(
            projector_thresholds([None, Some(11.), Some(20.)], Quality::Fair, 0., 0, 0, false)
                .unwrap(),
            [0, 6, 10]
        );
        assert_eq!(
            projector_thresholds([None, Some(11.), Some(20.)], Quality::Poor, 0., 0, 0, false)
                .unwrap(),
            [0, 0, 0]
        );
        assert!(projector_thresholds(
            [None, Some(11.), Some(20.)],
            Quality::Good,
            5.5,
            0,
            0,
            false
        )
        .is_err());
    }
}

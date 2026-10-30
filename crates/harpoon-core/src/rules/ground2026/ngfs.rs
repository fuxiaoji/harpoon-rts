//! §11.4 naval gunfire against land targets. All printed constants come from
//! data/rules/ngfs-2026.json (8-11/8-12/8-13 and Annex C, visually verified).
//! An unregistered gun keeps the registered "executor not bound" rejection
//! instead of the legacy range fraction.
//!
//! Unknown necessary inputs pend the resolution instead of applying an
//! implicit zero: visibility percent, sea state and the under-fire state must
//! be declared or observed, or the die is retained unrolled. A land target
//! cannot use radar GFC to ignore visibility (no beacon wording), so no such
//! ignore exists here.
//!
//! One attack per ship-gun per Tactical Turn. Own declarations collect first;
//! same-gun conflicts reject together before commitment. Canonical allocation
//! gives every ship the same once-only land concentration penalty.
//!
//! 11.6/11.7 whole-airfield and whole-port bombardment: the spotting task and
//! the concentration count belong to the DECLARED complex, and the
//! reverse-selected component never redefines the own spotting task. An
//! "empty space" reverse roll is a recorded final miss with no reroll; a
//! category without a declared component retains the die as pending
//! adjudication. Both stay in `State::retained` after the shared window.
//!
//! Ammunition: 8.3 leaves gun bookkeeping to the scenario. With
//! `track_ammo` the author-declared `ammo_per_attack` is checked before the
//! shot and debited once at shared commitment before the die; without
//! it nothing is debited. Rounds never buy extra attacks.
//!
//! Qualified hits are handed to the existing `aaw.surface.land_hits` common
//! LandHit path, so the 11.1.2 ratio rolls once per target per shared window
//! (including projectiles) and criticals cannot be lost to a leading miss.
//!
//! Shared ABI the root wires (this file does not touch shared files):
//! 1. `rules/ground2026.rs`: `pub mod ngfs;` plus the `ngfs` fields on
//!    Configuration/State and the initialize call (already in place).
//! 2. Phase order: commands (`validate_action`/`apply`, or `prepare_from_fire`
//!    from the weapons Base-target branch) -> the referee window runs
//!    `resolve_referee` -> `late_batch` inserts into land_hits before the
//!    common flush, which stays the single ratio window.
//! 3. `weapons.rs` keeps its rejection for unregistered guns; with a bound
//!    service it calls the read-only `validate_from_fire` then the actual
//!    `prepare_from_fire`. Normal Fire `rounds` never purchase extra attacks.
use crate::{EntityId, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub const NOT_BOUND: &str =
    "11 source gunnery/air-ordnance executor not yet bound; legacy range-fraction Pk forbidden";

pub fn tables() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../../data/rules/ngfs-2026.json")).unwrap()
    })
}

fn constant(key: &str) -> f64 {
    tables()["modifier_constants"][key]
        .as_f64()
        .unwrap_or_else(|| panic!("ngfs-2026.json modifier_constants missing {key}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GunneryStandard {
    Manual,
    Gs4,
    Gs5,
    Gs6,
}
impl GunneryStandard {
    fn key(self) -> &'static str {
        match self {
            GunneryStandard::Manual => "Manual",
            GunneryStandard::Gs4 => "GS4",
            GunneryStandard::Gs5 => "GS5",
            GunneryStandard::Gs6 => "GS6",
        }
    }
}

/// One Annex C shell row. Bands are kyds; the printed 8-11 example binds the
/// engine unit (7800 yd short range = "almost four nautical miles").
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GunBand {
    pub max_range_kyd: f64,
    pub belt_pen_cm: f64,
    /// Printed only from the Long band onward (B/D columns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deck_pen_cm: Option<f64>,
    pub damage: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GunSpec {
    pub name: String,
    pub shell: String,
    pub bands: Vec<GunBand>,
    pub aa_max_range_kyd: f64,
    pub aa_rating: f64,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub source: String,
    pub ship: String,
    /// Registered weapon id; must exist in `guns`.
    pub gun: String,
    /// Scenario-selected physical index; 0 preserves the legacy slot.
    #[serde(default, skip_serializing_if="is_zero")]
    pub physical_index:usize,
    /// Combat-system binding is an explicit scenario declaration.
    pub gunnery_standard: GunneryStandard,
    /// Annex C prints no barrel counts; the scenario declares the mount.
    pub barrels: u8,
    /// Shells consumed per attack; required whenever track_ammo is set and
    /// never inferred from barrels or rate of fire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ammo_per_attack: Option<u16>,
    /// 8.3: gun ammunition is tracked only when the scenario requires it.
    #[serde(default)]
    pub track_ammo: bool,
    /// Explicit facility target, or a registered complex for 11.6/11.7
    /// reverse-hit bombardment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complex: Option<String>,
    /// Necessary resolution input: printed +1 applies only when not under
    /// fire. Unset pends the resolution; nothing is assumed.
    pub ship_under_fire: Option<bool>,
    /// Necessary resolution input from the printed sea-state table. Unset
    /// pends the resolution.
    pub sea_state: Option<u8>,
    /// Size column of the firing vessel, not its number of barrels.
    #[serde(default)]
    pub sea_state_size_class: Option<char>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Configuration {
    pub source: String,
    #[serde(default)]
    pub guns: BTreeMap<String, GunSpec>,
    #[serde(default)]
    pub services: BTreeMap<String, ServiceConfig>,
}

/// Names resolved once at initialize so runtime matching can never silently
/// pick among same-named entities.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundService {
    pub ship: EntityId,
    pub side: crate::Side,
    pub gun: String,
    #[serde(default, skip_serializing_if="is_zero")]
    pub physical_index:usize,
    pub gunnery_standard: GunneryStandard,
    pub barrels: u8,
    pub ammo_per_attack: Option<u16>,
    pub track_ammo: bool,
    pub target: Option<EntityId>,
    pub complex: Option<EntityId>,
    pub ship_under_fire: Option<bool>,
    pub sea_state: Option<u8>,
    #[serde(default)]
    pub sea_state_size_class: Option<char>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub service: String,
    pub ship: EntityId,
    pub side: crate::Side,
    /// The declared entity: the facility itself, or the complex whose
    /// reverse-hit roll selects the actual component.
    pub declared: EntityId,
    pub gun: String,
    #[serde(default, skip_serializing_if="is_zero")]
    pub physical_index:usize,
    pub declared_tick: i64,
}
/// The 11.6/11.7 reverse roll of one complex bombardment.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReverseRoll {
    pub d100: u8,
    /// Selected category label, or "empty" for an automatic failure.
    pub component: String,
    /// The declared whole complex the attack was made against.
    pub declared: EntityId,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resolution {
    pub job: Job,
    /// The actual land entity this resolution is bound to after reverse-hit.
    pub land: EntityId,
    pub d100: u8,
    pub chance: f64,
    pub hit: bool,
    pub band: usize,
    pub penetrated: Option<bool>,
    pub damage: f64,
    pub pending: Option<String>,
    /// Present when the attack began with an 11.6/11.7 reverse roll. For
    /// outcomes that never reach a gunnery die (empty space, undeclared
    /// component) `d100` above is that reverse roll, not a gunnery die.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverse: Option<ReverseRoll>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub services: BTreeMap<String, BoundService>,
    pub jobs: Vec<Job>,
    pub resolutions: Vec<Resolution>,
    /// Batch attacks that never became a common LandHit: misses, empty
    /// reverse rolls and retained pending adjudications. Kept so a
    /// serialized state carries every unapplied die.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retained: Vec<Resolution>,
    /// One attack per ship-gun per Tactical Turn (180 s), ground2026 spot
    /// clock precedent.
    pub fired: BTreeSet<String>,
    pub diagnostics: BTreeSet<String>,
}

/// Typed order for explicit scenarios; the shared weapons path instead uses
/// `validate_from_fire` + `prepare_from_fire` with the same guarantees.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Fire { service: String },
}

fn trace(w: &mut World, id: EntityId, rule: &str, detail: Value) {
    super::super::aviation2026::trace(w, id, rule, detail);
}

fn named(w: &World, name: &str) -> Result<EntityId, String> {
    let matches: Vec<_> = w.entities.values().filter(|e| e.name == name).collect();
    if matches.len() != 1 {
        Err(format!("11.4 unit name must be unique: {name}"))
    } else {
        Ok(matches[0].id)
    }
}

fn is_zero(v:&usize)->bool{*v==0}
fn tac_key(ship: EntityId, gun: &str, physical:usize, tick: i64) -> String {
    if physical==0 {format!("{}:{}:{}",ship.0,gun,tick/180)} else {format!("{}:{}:physical:{}:{}",ship.0,gun,physical,tick/180)}
}
pub(crate) fn committed_physical(w:&World,ship:EntityId,gun:&str,physical:usize)->bool {
    w.rules.ground.as_ref().and_then(|s|s.ngfs.as_ref()).is_some_and(|s|s.fired.contains(&tac_key(ship,gun,physical,w.tick)))
}
pub(crate) fn committed(w:&World,ship:EntityId,gun:&str)->bool {committed_physical(w,ship,gun,0)}

fn band_of<'a>(gun: &'a GunSpec, range_nm: f64) -> Result<(usize, &'a GunBand), String> {
    let kyd = range_nm * 2.;
    for (i, band) in gun.bands.iter().enumerate() {
        if kyd <= band.max_range_kyd {
            return Ok((i, band));
        }
    }
    Err(format!(
        "11.4/Annex C range {kyd} kyd is beyond the printed extreme band; no extrapolation"
    ))
}

fn multiplier(barrels: u8, band: usize) -> Result<u32, String> {
    let rows = tables()["damage_multipliers_barrels_by_band"]["rows"]
        .as_array()
        .unwrap();
    let row = rows
        .iter()
        .find(|r| {
            let mut it = r["barrels"].as_str().unwrap().split('-');
            let lo: u32 = it.next().unwrap().parse().unwrap();
            let hi: u32 = it.next().unwrap().parse().unwrap();
            (lo..=hi).contains(&(barrels as u32))
        })
        .ok_or("11.4 barrels outside the printed 1-12 multiplier rows")?;
    Ok(row["mult"][band].as_u64().unwrap() as u32)
}

fn base_chance(standard: GunneryStandard, band: usize) -> f64 {
    tables()["base_chance_by_standard_and_band"][standard.key()][band]
        .as_f64()
        .unwrap()
}

fn scale(band: usize) -> f64 {
    let key = if band <= 1 { "short_medium" } else { "long_extreme" };
    tables()["base_chance_by_standard_and_band"]["band_scaling"][key]
        .as_f64()
        .unwrap()
}

pub fn initialize(w: &World, config: &Configuration) -> Result<State, String> {
    if config.source.trim().is_empty() {
        return Err("11.4 ngfs configuration requires a source label".into());
    }
    for (id, gun) in &config.guns {
        if gun.source.trim().is_empty() || gun.bands.is_empty() {
            return Err(format!("11.4/Annex C gun {id} needs bands and source"));
        }
        let mut last = 0.;
        for band in &gun.bands {
            if band.max_range_kyd <= last
                || band.max_range_kyd <= 0.
                || !band.max_range_kyd.is_finite()
                || band.damage < 0.
                || band.belt_pen_cm < 0.
            {
                return Err(format!("11.4/Annex C gun {id} bands must ascend with values"));
            }
            last = band.max_range_kyd;
        }
    }
    let mut services = BTreeMap::new();
    for (key, s) in &config.services {
        if s.source.trim().is_empty() {
            return Err(format!("11.4 service {key} requires a source label"));
        }
        let ship = named(w, &s.ship)?;
        let e = w.entity(ship).ok_or("11.4 firing ship absent")?;
        let side = e.side;
        if !e.weapons.iter().any(|m| m.weapon == s.gun) {
            return Err(format!("11.4 service {key} ship does not fit gun {}", s.gun));
        }
        let mount=e.weapons.iter().find(|m|m.weapon==s.gun).ok_or("11.4 mount missing")?;
        if s.physical_index>=usize::from(mount.mounts) {return Err("11.4 selected physical mount outside author-declared count".into());}
        config
            .guns
            .get(&s.gun)
            .ok_or_else(|| format!("11.4 service {key} gun {} has no registered Annex C bands", s.gun))?;
        if s.barrels == 0 || s.barrels > 12 {
            return Err(format!("11.4 service {key} barrels outside printed multiplier rows"));
        }
        if s.track_ammo && s.ammo_per_attack.is_none_or(|n| n == 0) {
            return Err(format!(
                "11.4 service {key} with track_ammo needs an explicit positive ammo_per_attack; shells per attack are not printed"
            ));
        }
        let target = match &s.target {
            Some(name) => {
                let id = named(w, name)?;
                super::source_target(w, id)?;
                Some(id)
            }
            None => None,
        };
        let complex = match &s.complex {
            Some(name) => {
                let id = named(w, name)?;
                if !w
                    .rules
                    .ground
                    .as_ref()
                    .is_some_and(|g| g.complexes.contains_key(&id))
                {
                    return Err(format!("11.4 service {key} complex is not configured"));
                }
                Some(id)
            }
            None => None,
        };
        if target.is_none() && complex.is_none() {
            return Err(format!("11.4 service {key} needs an explicit facility or complex"));
        }
        services.insert(
            key.clone(),
            BoundService {
                ship,
                side,
                gun: s.gun.clone(),
                physical_index:s.physical_index,
                gunnery_standard: s.gunnery_standard,
                barrels: s.barrels,
                ammo_per_attack: s.ammo_per_attack,
                track_ammo: s.track_ammo,
                target,
                complex,
                ship_under_fire: s.ship_under_fire,
                sea_state: s.sea_state,
                sea_state_size_class: s.sea_state_size_class,
            },
        );
    }
    Ok(State {
        services,
        ..State::default()
    })
}

/// Read-only precheck: what the actual shot would need. Writes nothing.
pub fn configured(w: &World, shooter: EntityId, weapon: &str) -> bool {
    w.rules.ground.as_ref().and_then(|s| s.ngfs.as_ref())
        .is_some_and(|s| s.services.values().any(|b| b.ship == shooter && b.gun == weapon))
}

pub fn validate_from_fire(
    w: &World,
    shooter: EntityId,
    target: EntityId,
    weapon: &str,
) -> Result<(), String> {
    let cfg = w
        .context
        .ground_2026
        .as_ref()
        .and_then(|c| c.ngfs.as_ref())
        .ok_or_else(|| NOT_BOUND.to_string())?;
    let state = w
        .rules
        .ground
        .as_ref()
        .and_then(|s| s.ngfs.as_ref())
        .ok_or_else(|| NOT_BOUND.to_string())?;
    let (key, bound) = state
        .services
        .iter()
        .find(|(_, b)| b.ship == shooter && (b.target == Some(target) || b.complex == Some(target)))
        .ok_or_else(|| NOT_BOUND.to_string())?;
    let _ = key;
    if bound.gun != weapon {
        return Err(format!("11.4 service binds gun {}, not {weapon}", bound.gun));
    }
    // 11.4: a declared spotting (or barrage) task precedes any fire.
    let spotting = w
        .rules
        .ground
        .as_ref()
        .and_then(|s| s.spotting.get(&shooter))
        .ok_or("11.4 declare a spotting or barrage task before NGFS")?;
    if spotting.target != target {
        return Err("11.4 spotting task is bound to another target".into());
    }
    if matches!(spotting.method,super::Spotting::Map|super::Spotting::Blind) {
        return Err("11.4 neutralization fire needs qualified area/effect rules; direct destruction damage is not a substitute".into());
    }
    let gun = cfg.guns.get(&bound.gun).ok_or("11.4/Annex C gun bands missing")?;
    let range_nm = w
        .entity(shooter)
        .zip(w.entity(target))
        .map(|(s, t)| s.pos.range_to(t.pos))
        .ok_or("11.4 firing geometry unavailable")?;
    band_of(gun, range_nm)?;
    if matches!(spotting.method,super::Spotting::Ship) && range_nm>=5. {
        return Err("11.4 self-spotted destruction fire must also be below ten kyds".into());
    }
    if bound.track_ammo {
        let need = bound.ammo_per_attack.unwrap_or(0);
        let have = w
            .entity(shooter)
            .and_then(|e| e.weapons.iter().find(|m| m.weapon == bound.gun))
            .map(|m| m.ammo)
            .unwrap_or(0);
        if have < need {
            return Err(format!(
                "11.4 declared {need} shells per attack exceed the {have} available"
            ));
        }
    }
    let entity=w.entity(shooter).ok_or("11.4 firing ship absent")?;
    let mount_index=entity.weapons.iter().position(|m|m.weapon==bound.gun).ok_or("11.4 mount missing")?;
    if !super::super::expanded_damage2026::physical_weapon_mounts(w,shooter,mount_index)?.contains(&bound.physical_index) {return Err("11.4 selected physical mount unavailable".into());}
    if bound.track_ammo {
        if super::super::expanded_damage2026::physical_weapon_stock(w,shooter,&bound.gun,mount_index,bound.physical_index)?.is_some_and(|n|n<bound.ammo_per_attack.unwrap()) {return Err("11.4 selected physical ammunition exhausted".into());}
    }
    if state.fired.contains(&tac_key(shooter, &bound.gun, bound.physical_index,w.tick))
        || super::super::surface2026::gunnery::committed_physical(w, shooter, &bound.gun,bound.physical_index) {
        return Err("11.4 this ship-gun has already fired in the current Tactical Turn".into());
    }
    Ok(())
}

/// Collect an own declaration. Conflicts are arbitrated together before any
/// slot/ammunition expenditure or referee target selection.
pub fn prepare_from_fire(
    w: &mut World,
    shooter: EntityId,
    target: EntityId,
    weapon: &str,
) -> Result<(), String> {
    validate_from_fire(w, shooter, target, weapon)?;
    let (bound, key) = {
        let state = w.rules.ground.as_ref().and_then(|s| s.ngfs.as_ref()).unwrap();
        let found = state
            .services
            .iter()
            .find(|(_, b)| b.ship == shooter && (b.target == Some(target) || b.complex == Some(target)))
            .map(|(k, b)| (b.clone(), k.clone()))
            .unwrap();
        found
    };
    let state = w.rules.ground.as_mut().and_then(|s| s.ngfs.as_mut()).unwrap();
    if state.jobs.iter().any(|j|j.service==key && j.ship==shooter && j.declared==target && j.declared_tick==w.tick) {return Ok(());}
    state.jobs.push(Job {
        service: key,
        ship: shooter,
        side: bound.side,
        declared: target,
        gun: bound.gun.clone(),
        physical_index:bound.physical_index,
        declared_tick: w.tick,
    });
    trace(w, shooter, "11.4.ngfs.prepare", json!({"gun": bound.gun, "tick": w.tick}));
    Ok(())
}

pub fn validate_action(w: &World, unit: EntityId, action: &Action) -> Result<(), String> {
    let Action::Fire { service } = action;
    let state = w
        .rules
        .ground
        .as_ref()
        .and_then(|s| s.ngfs.as_ref())
        .ok_or_else(|| NOT_BOUND.to_string())?;
    let bound = state
        .services
        .get(service)
        .ok_or_else(|| format!("11.4 service {service} not configured"))?;
    if bound.ship != unit {
        return Err("11.4 only the bound firing ship may order the service".into());
    }
    let declared = bound.target.or(bound.complex).ok_or("11.4 service target unbound")?;
    validate_from_fire(w, unit, declared, &bound.gun)
}

pub fn apply(w: &mut World, unit: EntityId, action: &Action) -> Result<(), String> {
    let Action::Fire { service } = action;
    let bound = {
        let state = w.rules.ground.as_ref().and_then(|s| s.ngfs.as_ref()).unwrap();
        state.services.get(service).cloned().ok_or_else(|| format!("11.4 service {service} not configured"))?
    };
    let declared = bound.target.or(bound.complex).ok_or("11.4 service target unbound")?;
    prepare_from_fire(w, unit, declared, &bound.gun)
}

/// Category label of a reverse roll, for resolution records. The
/// authoritative selection (including the multi-facility check) stays in
/// ground2026::bombardment_target.
fn reverse_label(w: &World, complex: EntityId, d100: u8) -> String {
    w.rules
        .ground
        .as_ref()
        .and_then(|s| s.complexes.get(&complex))
        .and_then(|c| super::reverse_component(c.kind, d100).ok())
        .flatten()
        .unwrap_or_else(|| "empty".into())
}

/// Referee window: reverse-hit selection, chance and outcome rolls. Hits are
/// applied later through the common land_hits path, so nothing here rolls the
/// 11.1.2 ratio.
pub fn resolve_referee(w: &mut World) {
    if !super::active(w) {
        return;
    }
    super::super::surface2026::gunnery::arbitrate_domains(w);
    let Some(state) = w.rules.ground.as_mut().and_then(|s| s.ngfs.as_mut()) else {
        return;
    };
    if state.jobs.is_empty() {
        return;
    }
    let jobs = std::mem::take(&mut state.jobs);
    let services = state.services.clone();
    let Some(cfg) = w.context.ground_2026.as_ref().and_then(|c| c.ngfs.clone()) else {
        return;
    };
    let mut grouped:BTreeMap<String,BTreeMap<(EntityId,String),Job>>=BTreeMap::new();
    for job in jobs {
        grouped.entry(tac_key(job.ship,&job.gun,job.physical_index,job.declared_tick)).or_default()
            .entry((job.declared,job.service.clone())).or_insert(job);
    }
    let mut jobs=Vec::new();
    for alternatives in grouped.into_values() {
        if alternatives.len()!=1 {
            for job in alternatives.values() {trace(w,job.ship,"11.4.ngfs.conflict",json!({"gun":job.gun,"reason":"conflicting ship-gun declarations in the shared phase; no slot or ammunition spent"}));}
            continue;
        }
        jobs.push(alternatives.into_values().next().unwrap());
    }
    let mut prepared=Vec::new();
    let mut firing_ships:BTreeMap<EntityId,BTreeSet<EntityId>>=BTreeMap::new();
    // A committed miss/pending reverse result still means shells were fired
    // at that declared area. Restore the same-Tac contributors retained by
    // earlier common batches, without taking another die or spending stock.
    if let Some(s)=w.rules.ground.as_ref().and_then(|g|g.ngfs.as_ref()) {
        for r in s.resolutions.iter().chain(s.retained.iter()).filter(|r|r.job.declared_tick.div_euclid(180)==w.tick.div_euclid(180)) {
            firing_ships.entry(r.job.declared).or_default().insert(r.job.ship);
        }
    }
    for job in jobs {
        let Some(bound) = services.get(&job.service).cloned() else {
            continue;
        };
        if let Err(reason)=validate_from_fire(w,job.ship,job.declared,&job.gun) {
            trace(w,job.ship,"11.4.ngfs.cancelled",json!({"reason":reason}));continue;
        }
        if !w.entity(job.ship).is_some_and(|e|e.alive && e.weapons.iter().any(|m|m.weapon==job.gun && m.mounts>0))
            || !super::super::expanded_damage2026::weapon_operational(w,job.ship,&job.gun).unwrap_or(false) {
            trace(w,job.ship,"11.4.ngfs.cancelled",json!({"reason":"actual ship-gun unavailable at commitment"}));continue;
        }
        if bound.track_ammo {
            let index=w.entity(job.ship).unwrap().weapons.iter().position(|m|m.weapon==job.gun).unwrap();
            if let Err(reason)=super::super::expanded_damage2026::spend_physical_weapon_stock(w,job.ship,&job.gun,index,job.physical_index,bound.ammo_per_attack.unwrap()) {
                trace(w,job.ship,"11.4.ngfs.cancelled",json!({"reason":reason}));continue;
            }
            w.entity_mut(job.ship).unwrap().weapons[index].ammo-=bound.ammo_per_attack.unwrap();
        }
        w.rules.ground.as_mut().and_then(|s|s.ngfs.as_mut()).unwrap().fired.insert(tac_key(job.ship,&job.gun,job.physical_index,w.tick));
        firing_ships.entry(job.declared).or_default().insert(job.ship);
        // 11.6/11.7: a whole-complex bombardment rolls its component first.
        let mut reverse = None;
        let land = if let Some(complex) = bound.complex {
            let identity = format!("ngfs:{}", job.service);
            // Same deterministic derivation as bombardment_target; recorded
            // on the resolution because empty/undeclared outcomes never
            // reach a gunnery die.
            let reverse_d100 = {
                let mut r = w.rng.derive(crate::Rng::hash_tag(
                    w.tick,
                    complex.0,
                    &format!("11.reverse:{identity}"),
                ));
                1 + (r.next_f64() * 100.) as u8
            };
            match super::bombardment_target(w, complex, &identity) {
                Ok(Some(component)) => {
                    reverse = Some(ReverseRoll {
                        d100: reverse_d100,
                        component: reverse_label(w, complex, reverse_d100),
                        declared: complex,
                    });
                    component
                }
                Ok(None) => {
                    // "Empty space ... automatically failed to hit anything":
                    // a final recorded miss, never rerolled.
                    trace(
                        w,
                        complex,
                        "11.4.ngfs.reverse_empty",
                        json!({"d100": reverse_d100}),
                    );
                    w.rules
                        .ground
                        .as_mut()
                        .and_then(|s| s.ngfs.as_mut())
                        .unwrap()
                        .resolutions
                        .push(Resolution {
                            job: job.clone(),
                            land: complex,
                            d100: reverse_d100,
                            chance: 0.,
                            hit: false,
                            band: 0,
                            penetrated: None,
                            damage: 0.,
                            pending: None,
                            reverse: Some(ReverseRoll {
                                d100: reverse_d100,
                                component: "empty".into(),
                                declared: complex,
                            }),
                        });
                    continue;
                }
                Err(reason) => {
                    // The rolled category has no declared component (or
                    // needs an explicit allocation): retain the die as
                    // pending adjudication instead of dropping the attack.
                    let component = reverse_label(w, complex, reverse_d100);
                    trace(
                        w,
                        job.ship,
                        "11.4.ngfs.unresolved_component",
                        json!({"d100": reverse_d100, "reason": reason.as_str()}),
                    );
                    w.rules
                        .ground
                        .as_mut()
                        .and_then(|s| s.ngfs.as_mut())
                        .unwrap()
                        .resolutions
                        .push(Resolution {
                            job: job.clone(),
                            land: complex,
                            d100: reverse_d100,
                            chance: 0.,
                            hit: false,
                            band: 0,
                            penetrated: None,
                            damage: 0.,
                            pending: Some(reason),
                            reverse: Some(ReverseRoll {
                                d100: reverse_d100,
                                component,
                                declared: complex,
                            }),
                        });
                    continue;
                }
            }
        } else {
            job.declared
        };
        prepared.push((job, bound, land, reverse));
    }
    // Allocate every reverse-hit target before counting firing ships. The
    // printed land penalty applies to each ship once, never only later jobs,
    // and it counts ships firing at the DECLARED target: two ships shelling
    // one airfield are "firing at that target" whatever components their
    // reverse rolls select.
    if let Some(g)=w.rules.ground.as_ref() {
        for (ship,spot) in &g.spotting {
            if spot.last_fire.is_some_and(|at|at/180==w.tick/180) {
                firing_ships.entry(spot.target).or_default().insert(*ship);
            }
        }
    }
    for (job,bound,land,reverse) in prepared {
        let Ok(target_data) = super::source_target(w, land) else {
            continue;
        };
        let Some(gun) = cfg.guns.get(&job.gun).cloned() else {
            continue;
        };
        let Some(ship_e) = w.entity(job.ship) else { continue };
        let ship_speed_kts = ship_e.speed_kts;
        let range_nm = ship_e.pos.range_to(w.entity(land).map(|t| t.pos).unwrap_or(ship_e.pos));
        let (band_idx, band) = match band_of(&gun, range_nm) {
            Ok(v) => v,
            Err(reason) => {
                trace(w, job.ship, "11.4.ngfs.pending", json!({"reason": reason}));
                continue;
            }
        };
        // The historical spotting producer already includes recent-ship
        // concentration. Remove that term and replace it with the shared
        // whole-window count; do not charge it a second time below.
        let old_concentration={
            let g=w.rules.ground.as_ref().unwrap();
            let own_recent=g.spotting.get(&job.ship).is_some_and(|s|s.last_fire.is_some_and(|at|w.tick-at<180));
            let n=g.spotting.values().filter(|s|s.target==job.declared && !matches!(s.method,super::Spotting::Map|super::Spotting::Blind) && s.last_fire.is_some_and(|at|w.tick-at<180)).count();
            (n+usize::from(!own_recent)).saturating_sub(1)
        };
        // The declared spotting task owns the modifier: the reverse-selected
        // component is where the shells land, never a new spotting target,
        // so the task on the declared complex is consumed here unchanged.
        let spotting_shift = match super::source_spotting(w, job.ship, job.declared, band_idx <= 1) {
            Ok(v) => v as f64+2.*old_concentration as f64,
            Err(reason) => {
                trace(w, job.ship, "11.4.ngfs.cancelled", json!({"reason": reason}));
                continue;
            }
        };
        // Necessary inputs without printed defaults pend the die instead of
        // applying an implicit zero.
        let visibility = w
            .context
            .aviation_2026
            .as_ref()
            .and_then(|c| c.visibility_percent);
        let mut pending: Option<String> = None;
        let mut shifts = spotting_shift;
        if bound.barrels<=10 {shifts+=((bound.barrels-1)/2) as f64;}
        else {pending=Some("11-12 barrel hit modifier unprinted; damage multiplier alone does not qualify the hit die".into());}
        if visibility.is_none() {
            pending = Some("visibility percent unknown; the printed visibility modifiers cannot be evaluated".into());
        }
        if bound.sea_state.is_none() {
            pending = Some("sea state undeclared; the printed sea-state modifiers cannot be evaluated".into());
        }
        if bound.ship_under_fire.is_none() {
            pending = Some("under-fire state undeclared; the printed not-under-fire modifier cannot be evaluated".into());
        }
        if bound.sea_state.is_some_and(|s|s>=3) && !matches!(bound.sea_state_size_class,Some('A'..='G')) {
            pending=Some("firing ship size class undeclared for the sea-state column".into());
        }
        let sea_state = bound.sea_state.unwrap_or(0);
        if pending.is_none() {
            if sea_state > 6 {
                trace(
                    w,
                    job.ship,
                    "11.4.ngfs.pending",
                    json!({"reason": "no fire possible above sea state 6"}),
                );
                continue;
            }
            let column = match bound.sea_state_size_class {Some('A'|'B')=>"AB",Some('C'|'D')=>"CD",_=>"EG"};
            match tables()["modifiers_relevant_to_11_4"]["sea_state_table"][sea_state.to_string()][column]
                .as_str()
            {
                Some("NFP") => {
                    trace(
                        w,
                        job.ship,
                        "11.4.ngfs.pending",
                        json!({"reason": "sea state no fire possible"}),
                    );
                    continue;
                }
                Some(v) => {
                    if let Ok(n) = v.parse::<f64>() {
                        shifts += n;
                    }
                }
                None => {}
            }
            if let Some(v)=tables()["modifiers_relevant_to_11_4"]["sea_state_table"][sea_state.to_string()][column].as_f64() {shifts+=v;}
            shifts += if ship_speed_kts > 30. {
                constant("firing_speed_above_30kts")
            } else {
                0.
            };
            if w.entity(land).is_some_and(|t| t.speed_kts == 0.) {
                shifts += constant("stationary_target");
            }
            if bound.ship_under_fire == Some(false) {
                shifts += constant("ship_not_under_fire");
            }
            if let Some(v) = visibility {
                if v <= 20. {
                    shifts += constant("visibility_20_or_less");
                } else if v <= 40. {
                    shifts += constant("visibility_40_or_less");
                }
            }
            // 11.4 land destruction-fire overconcentration, distinct from the
            // 8.3.1 ship rule inside source_spotting's own multi-spotter term.
            let others=firing_ships.get(&job.declared).map_or(0,|ships|ships.len().saturating_sub(1));
            shifts += constant("land_overconcentration_per_extra_ship") * others as f64;
        }
        let mut chance = base_chance(bound.gunnery_standard, band_idx) + shifts * scale(band_idx);
        let mut point_blank = false;
        if let Some(v) = visibility {
            let half_short_nm = gun.bands[0].max_range_kyd / 4.;
            if range_nm <= half_short_nm && v >= 30. {
                chance += constant("point_blank_chance") * scale(0);
                point_blank = true;
            }
        }
        chance = chance.clamp(
            0.,
            tables()["base_chance_by_standard_and_band"]["ceiling"]
                .as_f64()
                .unwrap(),
        );
        let mut r = w.rng.derive(crate::Rng::hash_tag(
            w.tick,
            job.ship.0,
            &format!("11.4.ngfs:{}:{}", job.service, land.0),
        ));
        let d100 = 1 + (r.next_f64() * 100.) as u8;
        let mut resolution = Resolution {
            hit: false,
            d100,
            chance,
            band: band_idx,
            penetrated: None,
            damage: 0.,
            pending: pending.clone(),
            job: job.clone(),
            land,
            reverse,
        };
        if resolution.pending.is_none() {
            // Existing ULP-aware comparison protects exact printed D100
            // boundaries; it adds no rule rounding or chance modifier.
            resolution.hit = super::super::damage_runtime::threshold_met(chance*100.,d100 as f64);
        }
        if resolution.hit {
            let big_structure = matches!(target_data.size, 'A' | 'B' | 'C' | 'D');
            let mut damage = band.damage;
            if big_structure {
                damage *= multiplier(bound.barrels, band_idx).unwrap_or(1) as f64;
            }
            if point_blank {
                damage *= constant("point_blank_damage_multiplier");
            }
            // Short/Medium print belt penetration only; Long/Extreme print the
            // belt/deck pair. Each printed value is checked against its own
            // armor face; unprinted combinations are not invented.
            let belt =
                target_data.vertical_armor == 0. || band.belt_pen_cm > target_data.vertical_armor;
            let deck_ok = match band.deck_pen_cm {
                Some(deck) => {
                    target_data.horizontal_armor == 0.
                        || deck > target_data.horizontal_armor
                }
                None => true,
            };
            let penetrated = if band.deck_pen_cm.is_some() {
                belt || deck_ok
            } else {
                belt
            };
            if !penetrated {
                resolution.pending = Some(
                    "11.4 nonpenetrating hit has no printed land damage rule; die retained".into(),
                );
            }
            resolution.penetrated = Some(penetrated);
            resolution.damage = if penetrated { damage } else { 0. };
        }
        trace(
            w,
            job.ship,
            "11.4.ngfs.resolution",
            json!({
                "service": job.service, "land": land, "d100": d100, "chance": chance,
                "band": band_idx, "hit": resolution.hit, "penetrated": resolution.penetrated,
                "damage": resolution.damage, "point_blank": point_blank, "shifts": shifts,
                "pending": resolution.pending,
                "reverse": resolution.reverse.as_ref().map(|v| json!({"d100": v.d100, "component": v.component}))}),
        );
        w.rules
            .ground
            .as_mut()
            .and_then(|s| s.ngfs.as_mut())
            .unwrap()
            .resolutions
            .push(resolution);
    }
}

/// Late batch: hand every qualified hit to the existing common LandHit path
/// so the 11.1.2 ratio rolls once per target in the shared window and a
/// leading miss or non-penetration can never drop the group's criticals.
/// Every unapplied attack — miss, empty reverse roll or pending
/// adjudication — is retained in the serialized state.
pub fn late_batch(w: &mut World) {
    if !super::active(w) {
        return;
    }
    if w
        .rules
        .ground
        .as_ref()
        .and_then(|s| s.ngfs.as_ref())
        .is_none_or(|s| s.resolutions.is_empty())
    {
        return;
    }
    let resolutions = std::mem::take(
        &mut w
            .rules
            .ground
            .as_mut()
            .and_then(|s| s.ngfs.as_mut())
            .unwrap()
            .resolutions,
    );
    let mut retained = Vec::new();
    for r in resolutions {
        if r.hit && r.pending.is_none() && r.damage > 0. {
            let Some(def) = w.db.weapons.get(&r.job.gun).cloned() else {
                retained.push(r);
                continue;
            };
            let hit_id = w.allocate_id();
            match w.rules.aaw.as_mut() {
                Some(aaw) => {
                    aaw.surface.land_hits.insert(
                        hit_id,
                        super::super::surface2026::LandHit {
                            target: r.land,
                            weapon: def,
                            shooter_side: r.job.side,
                            damage: r.damage,
                        },
                    );
                }
                None => trace(
                    w,
                    r.job.ship,
                    "11.4.ngfs.pending",
                    json!({"reason": "common land-hit ledger unavailable", "damage": r.damage}),
                ),
            }
        } else {
            if let Some(pending) = &r.pending {
                let rule = if r.penetrated == Some(false) {
                    "11.4.nonpenetration_pending"
                } else {
                    "11.4.adjudication_pending"
                };
                trace(
                    w,
                    r.job.ship,
                    rule,
                    json!({"pending": pending, "d100": r.d100}),
                );
            }
            retained.push(r);
        }
    }
    if !retained.is_empty() {
        w.rules
            .ground
            .as_mut()
            .and_then(|s| s.ngfs.as_mut())
            .unwrap()
            .retained
            .extend(retained);
    }
}

/// Own projection only: queued orders. Referee resolutions (dice, chances,
/// penetration internals) are never exposed here.
pub fn own_view(w: &World, id: EntityId) -> Option<Value> {
    let state = w.rules.ground.as_ref()?.ngfs.as_ref()?;
    let jobs: Vec<_> = state
        .jobs
        .iter()
        .filter(|j| j.ship == id)
        .map(|j| json!({"service": j.service, "gun": j.gun, "declared_tick": j.declared_tick}))
        .collect();
    if jobs.is_empty() {
        return None;
    }
    Some(json!({"ngfs_jobs": jobs}))
}

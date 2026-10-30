//! Explicit alongside services. The source rule is the exact berth casualty;
//! stock, capacity, geometry and time are labelled scenario operating inputs.
//! Requests settle together after movement and committed primary damage.
use super::{Action, State as GroundState};
use crate::{db::WeaponKind, types::PlatformKind, EntityId, EventKind, Side, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepotConfig {
    #[serde(default, skip_serializing_if = "DepotKind::independent")]
    pub kind: DepotKind,
    pub source: String,
    pub stocks: BTreeMap<String, u16>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepotKind { #[default] Independent, MagazineBunker }
impl DepotKind { fn independent(&self)->bool { *self==Self::Independent } }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceConfig {
    pub source: String,
    pub depot: String,
    pub berth: String,
    pub recipient: String,
    pub mount_index: usize,
    pub weapon: String,
    pub capacity: u16,
    pub radius_nm: f64,
    pub max_speed_kts: f64,
    pub seconds_per_round: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Depot {
    #[serde(default, skip_serializing_if = "DepotKind::independent")]
    pub kind: DepotKind,
    pub name: String,
    pub source: String,
    pub stocks: BTreeMap<String, u16>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Service {
    pub config: ServiceConfig,
    pub owner: Side,
    pub depot: EntityId,
    pub berth: EntityId,
    pub recipient: EntityId,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub unit: EntityId,
    pub rounds: u16,
    pub started: i64,
    pub due: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub unit: EntityId,
    pub service: String,
    /// None is an explicit cancellation, which wins the same phase.
    pub rounds: Option<u16>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub depots: BTreeMap<EntityId, Depot>,
    pub services: BTreeMap<String, Service>,
    pub jobs: BTreeMap<String, Job>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requests: Vec<Request>,
}

fn supported(kind: WeaponKind) -> bool {
    matches!(
        kind,
        WeaponKind::Gun | WeaponKind::Ciws | WeaponKind::Sam | WeaponKind::Ascm | WeaponKind::Slcm
    )
}
fn mount(w: &World, service: &Service) -> Result<u16, String> {
    let c = &service.config;
    let ship = w
        .entity(service.recipient)
        .ok_or("11.berth recipient missing")?;
    if ship.kind != PlatformKind::Ship {
        return Err("11.berth first service domain requires a surface ship".into());
    }
    let m = ship
        .weapons
        .get(c.mount_index)
        .ok_or("11.berth selected mount missing")?;
    if m.weapon != c.weapon
        || m.mounts != 1
        || ship.weapons.iter().filter(|m| m.weapon == c.weapon).count() != 1
    {
        return Err("11.berth exact single aggregate mount/weapon identity required".into());
    }
    if !w
        .db
        .weapons
        .get(&c.weapon)
        .is_some_and(|d| supported(d.kind))
    {
        return Err("11.berth special ASW/torpedo/air ordnance loading is not implemented".into());
    }
    if crate::rules::asw_weapons2026::is_projector_configured(w, service.recipient, &c.weapon) {
        return Err("11.berth projector physical pattern loading is not implemented".into());
    }
    let physical = w
        .rules
        .expanded_damage
        .as_ref()
        .and_then(|s| {
            s.config
                .units
                .get(&ship.name)
                .or_else(|| s.config.classes.get(&ship.class))
        })
        .is_some_and(|fit| {
            fit.components
                .values()
                .any(|c| c.kind == crate::rules::expanded_damage2026::ComponentKind::WeaponMount)
        });
    if physical {
        return Err("11.berth physical component ammunition loading is not implemented".into());
    }
    Ok(m.ammo)
}

pub(super) fn initialize(
    w: &World,
    ground: &GroundState,
    depots: &BTreeMap<String, DepotConfig>,
    services: &BTreeMap<String, ServiceConfig>,
) -> Result<Option<State>, String> {
    if depots.is_empty() && services.is_empty() {
        return Ok(None);
    }
    if depots.is_empty() || services.is_empty() {
        return Err(
            "11.berth explicit independent depot and service assignments are both required".into(),
        );
    }
    let mut state = State::default();
    for (name, depot) in depots {
        let id = super::find_named(w, name)?;
        match depot.kind {
            DepotKind::Independent if ground.facilities.contains_key(&id) => return Err("11.berth independent depot cannot use an unspecified Ground target service".into()),
            DepotKind::MagazineBunker if !ground.facilities.get(&id).is_some_and(|f|f.target.effect=="gun_ammo_only_d10_turns") => return Err("11.berth magazine service requires the exact Magazine Bunker target".into()),
            _ => {}
        }
        if depot.source.trim().is_empty()
            || depot.stocks.is_empty()
            || !w.entity(id).is_some_and(|e| e.alive && e.hp > 0.)
        {
            return Err(
                "11.berth live actual depot and explicitly sourced initial inventory required"
                    .into(),
            );
        }
        for weapon in depot.stocks.keys() {
            if !w.db.weapons.get(weapon).is_some_and(|d| supported(d.kind)) {
                return Err("11.berth source stock has unsupported or unknown ammunition".into());
            }
        }
        state.depots.insert(
            id,
            Depot {
                kind: depot.kind,
                name: name.clone(),
                source: depot.source.clone(),
                stocks: depot.stocks.clone(),
            },
        );
    }
    let mut capacities = BTreeMap::new();
    for (key, c) in services {
        if key.trim().is_empty()
            || c.source.trim().is_empty()
            || c.capacity == 0
            || c.seconds_per_round <= 0
            || !c.radius_nm.is_finite()
            || c.radius_nm < 0.
            || !c.max_speed_kts.is_finite()
            || c.max_speed_kts != 0.
        {
            return Err(
                "11.berth capacity/geometry/time require explicit inputs; first domain supports declared zero-speed alongside service only".into(),
            );
        }
        let depot = super::find_named(w, &c.depot)?;
        let berth = super::find_named(w, &c.berth)?;
        let recipient = super::find_named(w, &c.recipient)?;
        let side = w
            .entity(recipient)
            .ok_or("11.berth recipient missing")?
            .side;
        if ![depot, berth, recipient].iter().all(|id| {
            w.entity(*id)
                .is_some_and(|e| e.alive && e.hp > 0. && e.side == side)
        }) {
            return Err(
                "11.berth depot/berth/recipient must be live actual same-side entities".into(),
            );
        }
        if ![depot, berth]
            .iter()
            .all(|id| w.entity(*id).is_some_and(|e| e.kind == PlatformKind::Base))
        {
            return Err(
                "11.berth first service domain requires actual stationary depot/berth facilities"
                    .into(),
            );
        }
        if !ground
            .facilities
            .get(&berth)
            .is_some_and(|f| f.target.effect == "no_alongside_resupply")
        {
            return Err(
                "11.berth assignment requires an explicitly configured Ship Berth target".into(),
            );
        }
        if !state
            .depots
            .get(&depot)
            .is_some_and(|d| d.stocks.contains_key(&c.weapon))
        {
            return Err(
                "11.berth service requires its one explicitly declared depot/weapon stock".into(),
            );
        }
        if capacities
            .insert((recipient, c.mount_index), c.capacity)
            .is_some_and(|old| old != c.capacity)
        {
            return Err(
                "11.berth one destination mount cannot have conflicting declared capacities".into(),
            );
        }
        let service = Service {
            config: c.clone(),
            owner: side,
            depot,
            berth,
            recipient,
        };
        if mount(w, &service)? > c.capacity {
            return Err("11.berth initial ammunition exceeds declared destination capacity".into());
        }
        state.services.insert(key.clone(), service);
    }
    Ok(Some(state))
}

fn live(w: &World, state: &State, service: &Service) -> Result<(), String> {
    let c = &service.config;
    let recipient = w
        .entity(service.recipient)
        .ok_or("11.berth recipient missing")?;
    let side = service.owner;
    depot_available(w,state,service)?;
    for (id, name) in [
        (service.depot, &c.depot),
        (service.berth, &c.berth),
        (service.recipient, &c.recipient),
    ] {
        if super::find_named(w, name)? != id
            || !w
                .entity(id)
                .is_some_and(|e| e.alive && e.hp > 0. && e.side == side)
        {
            return Err(
                "11.berth own depot/berth/recipient unavailable or identity/side changed".into(),
            );
        }
    }
    if ![service.depot, service.berth].iter().all(|id| {
        w.entity(*id)
            .is_some_and(|e| e.kind == PlatformKind::Base && e.speed_kts == 0.)
    }) {
        return Err(
            "11.berth first domain requires stationary actual depot/berth facilities".into(),
        );
    }
    // The exact assigned berth is the source gate. No other child berth's
    // damage can disable this job just because it shares a parent.
    if !w
        .rules
        .ground
        .as_ref()
        .and_then(|g| g.facilities.get(&service.berth))
        .is_some_and(|f| f.target.effect == "no_alongside_resupply" && !f.disabled && !f.destroyed)
    {
        return Err("11.1 Ship Berth unavailable; no alongside resupply possible".into());
    }
    if !state
        .depots
        .get(&service.depot)
        .is_some_and(|d| d.name == c.depot && d.stocks.contains_key(&c.weapon))
    {
        return Err("11.berth original independent source stock missing".into());
    }
    let berth = w.entity(service.berth).ok_or("11.berth berth missing")?;
    if !recipient.speed_kts.is_finite()
        || recipient.speed_kts.abs() > c.max_speed_kts
        || !recipient.pos.range_to(berth.pos).is_finite()
        || recipient.pos.range_to(berth.pos) > c.radius_nm
    {
        return Err(
            "11.berth recipient departed declared alongside geometry or stopping envelope".into(),
        );
    }
    mount(w, service)?;
    Ok(())
}
fn depot_available(w:&World,state:&State,service:&Service)->Result<(),String>{
    let depot=state.depots.get(&service.depot).ok_or("11.berth source depot absent")?;
    let facility=w.rules.ground.as_ref().and_then(|g|g.facilities.get(&service.depot));
    match depot.kind {
        DepotKind::Independent => {
            if facility.is_some() || !super::operational(w,service.depot) {return Err("11.berth independent depot acquired an unsupported Ground target service".into())}
        },
        DepotKind::MagazineBunker => {
            let f=facility.filter(|f|f.target.effect=="gun_ammo_only_d10_turns").ok_or("11.berth exact Magazine Bunker source missing")?;
            if f.destroyed || f.target.damage_points.is_none_or(|dp|f.damage>=dp*0.5) {return Err("11.1 Magazine Bunker structurally unavailable; temporary clock cannot repair it".into())}
            if f.gun_only_until.is_some_and(|t|w.tick<t) && !w.db.weapons.get(&service.config.weapon).is_some_and(|d|matches!(d.kind,WeaponKind::Gun|WeaponKind::Ciws)) {return Err("11.1 Magazine Bunker supplies gun ammunition only during original D10 turns".into())}
        }
    }
    Ok(())
}
fn reserved_stock(state: &State, depot: EntityId, weapon: &str) -> u32 {
    state
        .jobs
        .iter()
        .filter_map(|(key, j)| {
            let s = state.services.get(key)?;
            (s.depot == depot && s.config.weapon == weapon).then_some(u32::from(j.rounds))
        })
        .sum()
}
fn reserved_capacity(state: &State, unit: EntityId, mount_index: usize) -> u32 {
    state
        .jobs
        .iter()
        .filter_map(|(key, j)| {
            let s = state.services.get(key)?;
            (s.recipient == unit && s.config.mount_index == mount_index)
                .then_some(u32::from(j.rounds))
        })
        .sum()
}
fn quantity(w: &World, state: &State, service: &Service, rounds: u16) -> Result<(), String> {
    if rounds == 0 {
        return Err("11.berth a positive integral transfer quantity is required".into());
    }
    let c = &service.config;
    let stock = state
        .depots
        .get(&service.depot)
        .and_then(|d| d.stocks.get(&c.weapon))
        .ok_or("11.berth source stock missing")?;
    if u32::from(*stock) < reserved_stock(state, service.depot, &c.weapon) + u32::from(rounds) {
        return Err("11.berth independent source stock already reserved or insufficient".into());
    }
    if u32::from(mount(w, service)?)
        + reserved_capacity(state, service.recipient, c.mount_index)
        + u32::from(rounds)
        > u32::from(c.capacity)
    {
        return Err("11.berth destination capacity already reserved or insufficient".into());
    }
    if c.seconds_per_round
        .checked_mul(i64::from(rounds))
        .and_then(|d| w.tick.checked_add(d))
        .is_none()
    {
        return Err("11.berth transfer completion time overflow".into());
    }
    Ok(())
}
fn state(w: &World) -> Result<&State, String> {
    if !super::active(w) {
        return Err("11.berth requires explicit hv2026-rts-v4".into());
    }
    w.rules
        .ground
        .as_ref()
        .and_then(|g| g.berth.as_ref())
        .ok_or_else(|| "11.berth no explicit independent service is configured".into())
}
fn selected<'a>(state: &'a State, unit: EntityId, key: &str) -> Result<&'a Service, String> {
    state
        .services
        .get(key)
        .filter(|s| s.recipient == unit)
        .ok_or_else(|| "11.berth service belongs to a different recipient or is unknown".into())
}

/// Public shared preflight; it never reserves stock or mutates a private job.
/// Old Ground variants retain their own original validation path.
pub fn validate_action(w: &World, unit: EntityId, action: &Action) -> Result<(), String> {
    match action {
        Action::AlongsideResupply { service, rounds } => {
            let state = state(w)?;
            let s = selected(state, unit, service)?;
            live(w, state, s)?;
            if state.jobs.contains_key(service) {
                return Err("11.berth this service is already running".into());
            }
            quantity(w, state, s, *rounds)
        }
        Action::CancelAlongsideResupply { service } => {
            // Idempotent cancel remains possible after a casualty; no current
            // berth availability is needed to release an owned reservation.
            let owned = selected(state(w)?, unit, service)?;
            if !w.entity(unit).is_some_and(|e| e.side == owned.owner) {
                // Match unknown/foreign-service denial; changed ownership
                // does not reveal or acquire the original private service.
                return Err(
                    "11.berth service belongs to a different recipient or is unknown".into(),
                );
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
pub(super) fn apply(w: &mut World, unit: EntityId, action: &Action) -> Result<(), String> {
    validate_action(w, unit, action)?;
    let (service, rounds) = match action {
        Action::AlongsideResupply { service, rounds } => (service.clone(), Some(*rounds)),
        Action::CancelAlongsideResupply { service } => (service.clone(), None),
        _ => return Err("11.berth non-service action".into()),
    };
    w.rules
        .ground
        .as_mut()
        .unwrap()
        .berth
        .as_mut()
        .unwrap()
        .requests
        .push(Request {
            unit,
            service,
            rounds,
        });
    Ok(())
}
fn trace(w: &mut World, unit: EntityId, owner: Option<Side>, rule: &str, detail: Value) {
    // Audit ownership is the original service, not the unit's current side.
    // Missing service identity is referee-only, including after entity cleanup.
    w.emit(
        EventKind::RulesEvent {
            domain: "ground".into(),
            rule: rule.into(),
            side: owner,
            unit: Some(unit),
            contact: None,
            detail: detail.to_string(),
        },
        owner.map_or(Visibility::Referee, Visibility::Side),
    );
}
fn cancel_job(w: &mut World, state: &mut State, key: &str, reason: &str) {
    if let Some(job) = state.jobs.remove(key) {
        trace(
            w,
            job.unit,
            state.services.get(key).map(|s| s.owner),
            "11.berth.transfer_cancelled",
            json!({"service":key,"rounds":job.rounds,"reason":reason,"ammunition_credited":0,"source_debited":0,"reservations_released":true}),
        );
    }
}

/// Root calls this once after this tick's movement and committed damage. This
/// phase sorts stable identities; client order never wins shared inventory.
pub fn step(w: &mut World) {
    if !super::active(w) {
        return;
    }
    let Some(mut state) = w.rules.ground.as_mut().and_then(|g| g.berth.take()) else {
        return;
    };
    let requests = std::mem::take(&mut state.requests);
    let mut cancels = BTreeSet::new();
    let mut starts: BTreeMap<(EntityId, String), BTreeSet<u16>> = BTreeMap::new();
    for r in requests {
        if selected(&state, r.unit, &r.service).is_err() {
            continue;
        }
        if let Some(n) = r.rounds {
            starts.entry((r.unit, r.service)).or_default().insert(n);
        } else {
            cancels.insert((r.unit, r.service));
        }
    }
    for (_, key) in &cancels {
        cancel_job(w, &mut state, key, "explicit same-phase cancellation");
    }
    // First cancel every physically invalid job, before any due transfer.
    let keys: Vec<_> = state.jobs.keys().cloned().collect();
    for key in keys {
        let result = state
            .services
            .get(&key)
            .ok_or_else(|| "11.berth service identity missing".to_string())
            .and_then(|s| live(w, &state, s));
        if let Err(reason) = result {
            cancel_job(w, &mut state, &key, &reason);
        }
    }
    let mut jobs: Vec<_> = state
        .jobs
        .iter()
        .map(|(key, job)| (job.unit, key.clone()))
        .collect();
    jobs.sort();
    for (_, key) in jobs {
        let Some(job) = state.jobs.remove(&key) else {
            continue;
        };
        let result = state
            .services
            .get(&key)
            .ok_or_else(|| "11.berth service missing".to_string())
            .and_then(|s| {
                if job.unit != s.recipient
                    || job.started > w.tick
                    || job.due < job.started
                    || job.due
                        != job
                            .started
                            .checked_add(
                                s.config
                                    .seconds_per_round
                                    .checked_mul(i64::from(job.rounds))
                                    .ok_or("11.berth invalid saved time")?,
                            )
                            .ok_or("11.berth invalid saved time")?
                {
                    return Err("11.berth invalid saved service job; no transfer".into());
                }
                live(w, &state, s)?;
                quantity(w, &state, s, job.rounds)
            });
        if let Err(reason) = result {
            trace(
                w,
                job.unit,
                state.services.get(&key).map(|s| s.owner),
                "11.berth.transfer_cancelled",
                json!({"service":key,"reason":reason,"ammunition_credited":0,"source_debited":0,"reservations_released":true}),
            );
            continue;
        }
        if job.due > w.tick {
            state.jobs.insert(key, job);
            continue;
        }
        let s = state.services[&key].clone();
        let stock = state
            .depots
            .get_mut(&s.depot)
            .unwrap()
            .stocks
            .get_mut(&s.config.weapon)
            .unwrap();
        let before_stock = *stock;
        let recipient = w.entity_mut(job.unit).unwrap();
        let ammo = &mut recipient.weapons[s.config.mount_index].ammo;
        let before_ammo = *ammo;
        // All fallible/identity checks precede these two ledger writes.
        *stock -= job.rounds;
        *ammo += job.rounds;
        trace(
            w,
            job.unit,
            Some(s.owner),
            "11.berth.transfer_completed",
            json!({"service":key,"depot":s.depot,"berth":s.berth,"weapon":s.config.weapon,"mount_index":s.config.mount_index,"rounds":job.rounds,"started":job.started,"due":job.due,"source_before":before_stock,"source_after":before_stock-job.rounds,"ammunition_before":before_ammo,"ammunition_after":before_ammo+job.rounds,"source":s.config.source}),
        );
    }
    for ((unit, key), amounts) in starts {
        if cancels.contains(&(unit, key.clone())) {
            continue;
        }
        let result = if amounts.len() != 1 {
            Err("11.berth conflicting same-phase quantities for one service".into())
        } else if state.jobs.contains_key(&key) {
            Err("11.berth this service is already running".into())
        } else {
            let rounds = *amounts.first().unwrap();
            selected(&state, unit, &key)
                .and_then(|s| {
                    live(w, &state, s)?;
                    quantity(w, &state, s, rounds)
                })
                .map(|_| rounds)
        };
        match result {
            Ok(rounds) => {
                let s = &state.services[&key];
                let due = w.tick + s.config.seconds_per_round * i64::from(rounds);
                state.jobs.insert(
                    key.clone(),
                    Job {
                        unit,
                        rounds,
                        started: w.tick,
                        due,
                    },
                );
                trace(
                    w,
                    unit,
                    Some(s.owner),
                    "11.berth.transfer_reserved",
                    json!({"service":key,"rounds":rounds,"started":w.tick,"due":due,"source_debited":0,"ammunition_credited":0,"source":s.config.source}),
                );
            }
            Err(reason) => trace(
                w,
                unit,
                state.services.get(&key).map(|s| s.owner),
                "11.berth.transfer_rejected",
                json!({"service":key,"reason":reason,"source_debited":0,"ammunition_credited":0}),
            ),
        }
    }
    w.rules.ground.as_mut().unwrap().berth = Some(state);
}

pub(super) fn own_view(w: &World, unit: EntityId) -> Option<Value> {
    let state = state(w).ok()?;
    let side = w.entity(unit)?.side;
    let mut services = BTreeMap::new();
    for (key, s) in &state.services {
        if s.recipient != unit
            || s.owner != side
            || ![s.depot, s.berth]
                .iter()
                .all(|id| w.entity(*id).is_some_and(|e| e.side == side))
        {
            continue;
        }
        let c = &s.config;
        services.insert(key, json!({
            "depot":s.depot,"berth":s.berth,"weapon":c.weapon,"mount_index":c.mount_index,
            "stock":state.depots.get(&s.depot).and_then(|d|d.stocks.get(&c.weapon)),
            "reserved_stock":reserved_stock(state,s.depot,&c.weapon),"capacity":c.capacity,
            "reserved_capacity":reserved_capacity(state,unit,c.mount_index),"job":state.jobs.get(key),
            "pending":live(w,state,s).err(),"source":c.source,"radius_nm":c.radius_nm,
            "max_speed_kts":c.max_speed_kts,"seconds_per_round":c.seconds_per_round
        }));
    }
    (!services.is_empty()).then(|| json!({"services":services}))
}

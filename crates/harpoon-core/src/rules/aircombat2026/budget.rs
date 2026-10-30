//! Shared source9.6 aircraft admission and release barriers. No weapon scan.
use crate::types::PlatformKind;
use crate::{EntityId, World};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AirAttackKind {
    UnguidedBomb,
    Pgm,
    Rocket,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claim {
    pub identity: String,
    pub actor: EntityId,
    pub target: EntityId,
    pub known_kind: PlatformKind,
    pub kind: AirAttackKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Pending,
    Reserved,
    Conflict,
    ShipLimit,
    AlreadyUsed,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ticket {
    pub identity: String,
    pub actor: EntityId,
    pub target: EntityId,
    pub turn: i64,
    pub epoch: u64,
    pub at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub claim: Claim,
    pub submitted_at: i64,
    pub source_turn: i64,
    pub decision: Decision,
    pub active: bool,
    pub ticket: Option<Ticket>,
    pub consumed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadyOffer {
    pub at: i64,
    pub epoch: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub records: BTreeMap<String, Record>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub planned_ships: BTreeMap<String, BTreeSet<EntityId>>,
    pub drafts: BTreeSet<String>,
    pub offers: BTreeMap<String, ReadyOffer>,
    pub closed_declarations_at: Option<i64>,
    pub closed_ready_at: Option<i64>,
    pub cancellations: BTreeMap<EntityId, i64>,
}
fn turn(w: &World) -> i64 {
    w.tick.div_euclid(
        super::tables()["surface_attack"]["turn_seconds"]
            .as_i64()
            .unwrap(),
    )
}
fn key(target: EntityId, turn: i64) -> String {
    format!("{}:{turn}", target.0)
}
fn take(w: &mut World) -> Option<State> {
    w.rules
        .aircombat
        .as_mut()
        .map(|s| std::mem::take(&mut s.attack_budget))
}
fn put(w: &mut World, state: State) {
    if let Some(s) = w.rules.aircombat.as_mut() {
        s.attack_budget = state;
    }
}
pub fn claim_identity(kind: AirAttackKind, actor: EntityId, at: i64, request: &Value) -> String {
    format!("9.6.claim:{kind:?}:{}:{at}:{}", actor.0, request)
}
pub fn decision(w: &World, identity: &str) -> Decision {
    w.rules
        .aircombat
        .as_ref()
        .and_then(|s| s.attack_budget.records.get(identity))
        .map(|r| r.decision)
        .unwrap_or(Decision::Pending)
}
pub fn submit(w: &mut World, claim: Claim) -> Result<(), String> {
    if !matches!(claim.known_kind, PlatformKind::Ship | PlatformKind::Base)
        || claim.identity.is_empty()
    {
        return Err("9.6 budget requires a canonical own-known ship/base claim".into());
    }
    let at = w.tick;
    let source_turn = turn(w);
    let state = &mut w
        .rules
        .aircombat
        .as_mut()
        .ok_or("9.6 air attack budget unavailable")?
        .attack_budget;
    if let Some(old) = state.records.get(&claim.identity) {
        if serde_json::to_value(&old.claim).unwrap() != serde_json::to_value(&claim).unwrap() {
            return Err("9.6 immutable attack identity was reused for different input".into());
        }
        return Ok(());
    }
    if state.closed_declarations_at == Some(at) {
        return Err("9.6 attack declaration barrier already closed for this tick".into());
    }
    state.drafts.insert(claim.identity.clone());
    state.records.insert(
        claim.identity.clone(),
        Record {
            claim,
            submitted_at: at,
            source_turn,
            decision: Decision::Pending,
            active: false,
            ticket: None,
            consumed: false,
        },
    );
    Ok(())
}
/// One physical release may intentionally attack several ships in a declared
/// coverage. These are captured own-known targets, never hidden collateral.
pub fn submit_planned(
    w: &mut World,
    claim: Claim,
    planned_ships: BTreeSet<EntityId>,
) -> Result<(), String> {
    if let Some(state) = w.rules.aircombat.as_ref().map(|s| &s.attack_budget) {
        if state.records.contains_key(&claim.identity)
            && state
                .planned_ships
                .get(&claim.identity)
                .cloned()
                .unwrap_or_default()
                != planned_ships
        {
            return Err(
                "9.6 immutable physical attack identity reused with different planned ships".into(),
            );
        }
    }
    let identity = claim.identity.clone();
    submit(w, claim)?;
    if !planned_ships.is_empty() {
        w.rules
            .aircombat
            .as_mut()
            .unwrap()
            .attack_budget
            .planned_ships
            .insert(identity, planned_ships);
    }
    Ok(())
}
fn ships(state: &State, claim: &Claim) -> BTreeSet<EntityId> {
    let mut targets = state
        .planned_ships
        .get(&claim.identity)
        .cloned()
        .unwrap_or_default();
    if claim.known_kind == PlatformKind::Ship {
        targets.insert(claim.target);
    }
    targets
}
/// Existing native passes in pre-budget saves keep their previously reserved opportunity.
/// This imports trusted persisted own request data, never target truth or an event.
pub(super) fn adopt_reserved(w: &mut World, claim: Claim, accepted_at: i64) {
    let source_turn = accepted_at.div_euclid(
        super::tables()["surface_attack"]["turn_seconds"]
            .as_i64()
            .unwrap(),
    );
    if let Some(s) = w.rules.aircombat.as_mut() {
        s.attack_budget
            .records
            .entry(claim.identity.clone())
            .or_insert(Record {
                claim,
                submitted_at: accepted_at,
                source_turn,
                decision: Decision::Reserved,
                active: true,
                ticket: None,
                consumed: false,
            });
    }
}
pub fn abort(w: &mut World, identity: &str) {
    if let Some(s) = w.rules.aircombat.as_mut() {
        if let Some(r) = s.attack_budget.records.get_mut(identity) {
            r.active = false;
            r.ticket = None;
            r.decision = Decision::Cancelled;
        }
        s.attack_budget.drafts.remove(identity);
        s.attack_budget.offers.remove(identity);
    }
}
pub fn finish(w: &mut World, identity: &str) {
    if let Some(s) = w.rules.aircombat.as_mut() {
        if let Some(r) = s.attack_budget.records.get_mut(identity) {
            r.active = false;
            r.ticket = None;
        }
        s.attack_budget.offers.remove(identity);
    }
}
pub fn cancel_actor(w: &mut World, actor: EntityId) {
    let at = w.tick;
    if let Some(s) = w.rules.aircombat.as_mut() {
        s.attack_budget.cancellations.insert(actor, at);
    }
    let ids: Vec<_> = w
        .rules
        .aircombat
        .as_ref()
        .map(|s| {
            s.attack_budget
                .records
                .iter()
                .filter(|(_, r)| r.claim.actor == actor && !r.consumed)
                .map(|(id, _)| id.clone())
                .collect()
        })
        .unwrap_or_default();
    for id in ids {
        abort(w, &id);
    }
}
pub fn close_declarations(w: &mut World) {
    let Some(mut state) = take(w) else { return };
    if state.closed_declarations_at == Some(w.tick) {
        put(w, state);
        return;
    }
    state.closed_declarations_at = Some(w.tick);
    let current = turn(w);
    let ids = std::mem::take(&mut state.drafts);
    let mut actors: BTreeMap<EntityId, Vec<String>> = BTreeMap::new();
    for id in ids {
        if state
            .records
            .get(&id)
            .is_some_and(|r| r.decision == Decision::Pending)
        {
            actors
                .entry(state.records[&id].claim.actor)
                .or_default()
                .push(id);
        }
    }
    let mut candidates = Vec::new();
    for (actor, ids) in actors {
        let prior = w
            .rules
            .aircombat
            .as_ref()
            .is_some_and(|s| s.surface_last_turn.get(&actor) == Some(&current))
            || state
                .records
                .values()
                .any(|r| r.active && r.claim.actor == actor);
        let reject = if state.cancellations.get(&actor) == Some(&w.tick) {
            Some(Decision::Cancelled)
        } else if ids.len() != 1 {
            Some(Decision::Conflict)
        } else if prior {
            Some(Decision::AlreadyUsed)
        } else {
            None
        };
        if let Some(reason) = reject {
            for id in ids {
                state.records.get_mut(&id).unwrap().decision = reason;
            }
        } else {
            candidates.extend(ids);
        }
    }
    let mut groups: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();
    for id in &candidates {
        let c = &state.records[id].claim;
        for target in ships(&state, c) {
            groups.entry(target).or_default().insert(c.actor);
        }
    }
    let mut rejected = BTreeSet::new();
    let max = super::tables()["surface_attack"]["maximum_aircraft_per_turn"]
        .as_u64()
        .unwrap() as usize;
    for (target, actors) in groups {
        let mut occupied = w
            .rules
            .aircombat
            .as_ref()
            .and_then(|s| s.surface_attacks.get(&key(target, current)))
            .cloned()
            .unwrap_or_default();
        occupied.extend(
            state
                .records
                .values()
                .filter(|r| r.active && ships(&state, &r.claim).contains(&target))
                .map(|r| r.claim.actor),
        );
        occupied.extend(actors);
        if occupied.len() > max {
            rejected.insert(target);
        }
    }
    for id in candidates {
        let targets = ships(&state, &state.records[&id].claim);
        let r = state.records.get_mut(&id).unwrap();
        if targets.iter().any(|target| rejected.contains(target)) {
            r.decision = Decision::ShipLimit;
            continue;
        }
        r.decision = Decision::Reserved;
        r.active = true;
        if let Some(s) = w.rules.aircombat.as_mut() {
            s.surface_last_turn.insert(r.claim.actor, current);
            for target in targets.into_iter().chain(std::iter::once(r.claim.target)) {
                s.surface_attacks
                    .entry(key(target, current))
                    .or_default()
                    .insert(r.claim.actor);
            }
        }
    }
    put(w, state);
}
pub fn offer_ready(w: &mut World, identity: &str, epoch: u64) -> Result<(), String> {
    let at = w.tick;
    let s = w
        .rules
        .aircombat
        .as_mut()
        .ok_or("9.6 air attack budget unavailable")?;
    if s.surface.scan_open || s.surface.scan_epoch != epoch {
        return Err("9.6 release offer needs this tick's completed normal defense scan".into());
    }
    let state = &mut s.attack_budget;
    if state.closed_ready_at == Some(at) {
        return Err("9.6 ready barrier already closed".into());
    }
    let r = state
        .records
        .get(identity)
        .ok_or("9.6 immutable attack claim absent")?;
    if r.decision != Decision::Reserved || !r.active || r.consumed {
        return Err("9.6 attack claim has no active reservation".into());
    }
    state
        .offers
        .insert(identity.into(), ReadyOffer { at, epoch });
    Ok(())
}
pub fn close_ready(w: &mut World) {
    let Some(mut state) = take(w) else { return };
    if state.closed_ready_at == Some(w.tick) {
        put(w, state);
        return;
    }
    state.closed_ready_at = Some(w.tick);
    let current = turn(w);
    let epoch = w
        .rules
        .aircombat
        .as_ref()
        .map(|s| s.surface.scan_epoch)
        .unwrap_or(0);
    for r in state.records.values_mut() {
        r.ticket = None;
    }
    let offers = std::mem::take(&mut state.offers);
    let mut eligible = Vec::new();
    for (id, offer) in offers {
        if offer.at == w.tick
            && offer.epoch == epoch
            && state
                .records
                .get(&id)
                .is_some_and(|r| r.active && !r.consumed && r.decision == Decision::Reserved)
        {
            eligible.push(id);
        }
    }
    let mut groups: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();
    for id in &eligible {
        let c = &state.records[id].claim;
        for target in ships(&state, c) {
            groups.entry(target).or_default().insert(c.actor);
        }
    }
    let mut blocked = BTreeSet::new();
    let max = super::tables()["surface_attack"]["maximum_aircraft_per_turn"]
        .as_u64()
        .unwrap() as usize;
    for (target, actors) in groups {
        let mut occupied = w
            .rules
            .aircombat
            .as_ref()
            .and_then(|s| s.surface_attacks.get(&key(target, current)))
            .cloned()
            .unwrap_or_default();
        occupied.extend(actors);
        if occupied.len() > max {
            blocked.insert(target);
        }
    }
    for id in eligible {
        let targets = ships(&state, &state.records[&id].claim);
        let r = state.records.get_mut(&id).unwrap();
        if targets.iter().any(|target| blocked.contains(target)) {
            continue;
        }
        r.ticket = Some(Ticket {
            identity: id,
            actor: r.claim.actor,
            target: r.claim.target,
            turn: current,
            epoch,
            at: w.tick,
        });
        if let Some(s) = w.rules.aircombat.as_mut() {
            s.surface_last_turn.insert(r.claim.actor, current);
            for target in targets.into_iter().chain(std::iter::once(r.claim.target)) {
                s.surface_attacks
                    .entry(key(target, current))
                    .or_default()
                    .insert(r.claim.actor);
            }
        }
    }
    put(w, state);
}
pub fn take_ticket(w: &mut World, identity: &str) -> Option<Ticket> {
    let current = turn(w);
    let at = w.tick;
    let s = w.rules.aircombat.as_mut()?;
    let r = s.attack_budget.records.get_mut(identity)?;
    let ticket = r.ticket.as_ref()?;
    if r.consumed
        || !r.active
        || ticket.at != at
        || ticket.turn != current
        || ticket.epoch != s.surface.scan_epoch
        || s.surface.scan_open
    {
        return None;
    }
    r.consumed = true;
    r.active = false;
    r.ticket.take()
}

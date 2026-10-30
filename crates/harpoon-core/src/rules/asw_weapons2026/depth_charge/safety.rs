//! §8.5.4.3 own-ship exposures. Physical releases, not submarine hits, create
//! this ledger. All victims join the existing common Underwater damage batch.
use super::Pattern;
use crate::{EntityId, EventKind, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    records: BTreeMap<String, Record>,
    first: BTreeMap<String, First>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pending: BTreeSet<String>,
    /// One exposure to the first ship's pattern per actual following ship,
    /// target and Tactical Turn. Repeated checks never manufacture extra DP.
    following_applied: BTreeMap<String, String>,
}
impl State {
    pub fn is_empty(&self) -> bool {
        self.records.is_empty() && self.first.is_empty() && self.following_applied.is_empty()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Record {
    actor: EntityId,
    target: EntityId,
    turn: i64,
    tick: i64,
    weapon: String,
    major_dp: f64,
    release_speed_kts: Option<f64>,
    ordered: bool,
    low: Status,
    following: Status,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Status {
    Unresolved,
    Clear,
    Queued {
        identity: String,
        reason: String,
        dp: f64,
    },
    AlreadyExposed {
        original_attack: String,
    },
    Pending {
        reason: String,
    },
    NoLiveVictim,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum First {
    Known {
        actor: EntityId,
        attack: String,
        weapon: String,
        major_dp: f64,
        tick: i64,
    },
    Pending {
        tick: i64,
        reason: String,
    },
}
fn source_turn(tick: i64) -> i64 {
    tick.div_euclid(super::field("safety_tactical_seconds") as i64)
}
fn key(target: EntityId, turn: i64) -> String {
    format!("{}:{turn}", target.0)
}
fn trace(w: &mut World, actor: EntityId, attack: &str, kind: &str, status: &Status) {
    w.emit(EventKind::RulesEvent{domain:"asw_weapons2026".into(),rule:"8.5.4.3.safety".into(),side:None,unit:Some(actor),contact:None,
        detail:json!({"attack":attack,"hazard":kind,"resolution":status,"source":"English2026 PDF101/8-21 section8.5.4.3"}).to_string()},Visibility::Referee);
}
fn update(w: &mut World, actor: EntityId, attack: &str, kind: &str, old: &mut Status, new: Status) {
    if *old != new {
        trace(w, actor, attack, kind, &new);
        *old = new;
    }
}
pub(super) fn record_drop(w: &World, s: &mut State, p: &Pattern) {
    s.pending.insert(p.id.clone());
    s.records.entry(p.id.clone()).or_insert_with(|| Record {
        actor: p.actor,
        target: p.target,
        turn: source_turn(p.tick),
        tick: p.tick,
        weapon: p.weapon.clone(),
        major_dp: p.damage_major,
        release_speed_kts: w
            .entity(p.actor)
            .map(|e| e.speed_kts)
            .filter(|v| v.is_finite() && *v >= 0.),
        ordered: false,
        low: Status::Unresolved,
        following: Status::Unresolved,
    });
}
/// All current-tick drops are present before chronological first selection.
/// A genuine same-tick tie is not resolved by network or EntityId order.
fn order_releases(s: &mut State) {
    let mut new: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for id in &s.pending {
        let r = &s.records[id];
        if !r.ordered {
            new.entry(key(r.target, r.turn))
                .or_default()
                .push(id.clone());
        }
    }
    for (group, ids) in new {
        let earliest = ids.iter().map(|id| s.records[id].tick).min().unwrap();
        let mut initial: Vec<_> = ids
            .iter()
            .filter(|id| s.records[*id].tick == earliest)
            .cloned()
            .collect();
        let previous = s.first.get(&group).cloned();
        if let Some(First::Known { attack, tick, .. }) = &previous {
            if *tick == earliest {
                initial.push(attack.clone());
            }
        }
        if previous
            .as_ref()
            .is_none_or(|p| matches!(p,First::Known{tick,..}if *tick>=earliest))
        {
            let actors: BTreeSet<_> = initial.iter().map(|id| s.records[id].actor).collect();
            let weapons: BTreeSet<_> = initial
                .iter()
                .map(|id| s.records[id].weapon.clone())
                .collect();
            let entry = if previous
                .as_ref()
                .is_some_and(|p| matches!(p,First::Known{tick,..}if *tick>earliest))
            {
                First::Pending {
                    tick: earliest,
                    reason: "original release chronology changed; no retroactive first-ship guess"
                        .into(),
                }
            } else if actors.len() != 1 {
                First::Pending{tick:earliest,reason:"distinct ships first released in the same tick; original source gives no first-ship tie-break".into()}
            } else if weapons.len() != 1 {
                First::Pending{tick:earliest,reason:"one first ship released different charge types at the same time; source charge precedence unresolved".into()}
            } else {
                let id = &initial[0];
                let r = &s.records[id];
                First::Known {
                    actor: r.actor,
                    attack: id.clone(),
                    weapon: r.weapon.clone(),
                    major_dp: r.major_dp,
                    tick: r.tick,
                }
            };
            s.first.insert(group, entry);
        }
        for id in ids {
            s.records.get_mut(&id).unwrap().ordered = true;
        }
    }
}
fn settled(s: &Status) -> bool {
    matches!(
        s,
        Status::Clear
            | Status::Queued { .. }
            | Status::AlreadyExposed { .. }
            | Status::NoLiveVictim
    )
}
fn queue(
    w: &mut World,
    actor: EntityId,
    weapon: &str,
    dp: f64,
    attack: &str,
    reason: &str,
) -> Result<Status, String> {
    let identity = format!("{attack}:safety:{reason}");
    super::super::super::damage_runtime::queue_depth_charge_self_blast(
        w, actor, weapon, dp, &identity, reason,
    )?;
    Ok(Status::Queued {
        identity,
        reason: reason.into(),
        dp,
    })
}
/// Runs after all declarations/own final fuze selections, before target
/// capture/adjudication. A miss, absent submarine, or D100 cannot suppress it.
pub(super) fn resolve(w: &mut World, s: &mut State, patterns: &BTreeMap<String, Pattern>) {
    order_releases(s);
    let ids: Vec<_> = s.pending.iter().cloned().collect();
    for id in ids {
        let Some(p) = patterns.get(&id) else { continue };
        let mut r = s.records.remove(&id).unwrap();
        if settled(&r.low) && settled(&r.following) {
            s.pending.remove(&id);
            s.records.insert(id, r);
            continue;
        }
        if !w
            .entity(r.actor)
            .is_some_and(|e| e.alive && e.kind == crate::types::PlatformKind::Ship)
        {
            if !settled(&r.low) {
                update(
                    w,
                    r.actor,
                    &id,
                    "low_speed",
                    &mut r.low,
                    Status::NoLiveVictim,
                );
            }
            if !settled(&r.following) {
                update(
                    w,
                    r.actor,
                    &id,
                    "following_ship",
                    &mut r.following,
                    Status::NoLiveVictim,
                );
            }
            s.pending.remove(&id);
            s.records.insert(id, r);
            continue;
        }
        let low = match r.release_speed_kts {
            Some(speed) if speed >= super::field("safety_minimum_speed_kts") => Some(false),
            Some(_) if p.depth_selected => Some(
                super::tables()["depth_charge"]["safety_low_speed_bands"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v.as_u64() == Some(p.declared_depth as u64)),
            ),
            _ => None,
        };
        if !settled(&r.low) {
            let resolution=match low {
                None=>Status::Pending{reason:"original release speed or final own fuze depth unresolved; no guessed low-speed exposure".into()},
                Some(false)=>Status::Clear,
                Some(true)=>queue(w,r.actor,&r.weapon,r.major_dp,&id,"low_speed").unwrap_or_else(|reason|Status::Pending{reason}),
            };
            update(w, r.actor, &id, "low_speed", &mut r.low, resolution);
        }
        if !settled(&r.following) {
            let exposure = format!("{}:{}:{}", r.target.0, r.turn, r.actor.0);
            let first = s.first.get(&key(r.target, r.turn)).cloned();
            let resolution = match first {
                None => Status::Pending {
                    reason: "original first ship not recorded".into(),
                },
                Some(First::Pending { reason, .. }) => Status::Pending { reason },
                Some(First::Known { actor, .. }) if actor == r.actor => Status::Clear,
                Some(First::Known {
                    weapon,
                    major_dp,
                    attack,
                    ..
                }) => {
                    if let Some(original) = s.following_applied.get(&exposure) {
                        Status::AlreadyExposed {
                            original_attack: original.clone(),
                        }
                    } else if low != Some(false) {
                        Status::Pending{reason:"additional following-ship exposure overlaps unresolved/positive low-speed exposure; stacking priority not source-verified".into()}
                    } else {
                        match queue(w, r.actor, &weapon, major_dp, &id, "following_ship") {
                            Ok(status) => {
                                s.following_applied.insert(exposure, attack);
                                status
                            }
                            Err(reason) => Status::Pending { reason },
                        }
                    }
                }
            };
            update(
                w,
                r.actor,
                &id,
                "following_ship",
                &mut r.following,
                resolution,
            );
        }
        if settled(&r.low) && settled(&r.following) {
            s.pending.remove(&id);
        }
        s.records.insert(id, r);
    }
}
pub(super) fn own_view(s: &State, actor: EntityId) -> Value {
    let own:Vec<_>=s.records.iter().filter(|(_,r)|r.actor==actor).map(|(id,r)|json!({"attack":id,"release_speed_kts":r.release_speed_kts,"low_speed":r.low,"following_ship":match &r.following{Status::AlreadyExposed{..}=>json!({"status":"already_exposed"}),other=>json!(other)}})).collect();
    if own.is_empty() {
        Value::Null
    } else {
        json!(own)
    }
}

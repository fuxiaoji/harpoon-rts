//! Native §9.6 pass sequencing. One normal FCS scan; no hidden admission oracle.
use super::{budget, Bombsight, Delivery};
use crate::rules::{aviation2026 as av, expanded_damage2026 as ed};
use crate::types::PlatformKind;
use crate::{ContactId, EntityId, EventKind, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SurfaceDefenseKind {
    Gun { source_aa_rating: f64 },
    Sam { projectiles: Vec<EntityId> },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DefenseRecord {
    pub at: i64,
    pub aircraft: EntityId,
    pub shooter: EntityId,
    pub kind: SurfaceDefenseKind,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnresolvedDefense {
    pub at: i64,
    pub aircraft: EntityId,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NativeAim {
    pub observer: EntityId,
    pub sensor: String,
    pub at: i64,
    pub measured_position: Vec2,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReleaseAnchor {
    pub at: i64,
    pub position: Vec2,
    pub course_deg: f64,
    pub altitude_m: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedRelease {
    pub actor: EntityId,
    pub side: Side,
    pub class: String,
    pub contact: ContactId,
    pub target: EntityId,
    pub known_kind: PlatformKind,
    pub weapon: String,
    pub rounds: u16,
    pub delivery: Delivery,
    pub aimpoint: String,
    pub aim_source: String,
    pub point: Vec2,
    pub no_aa_source: Option<String>,
    pub native: Option<NativeAim>,
    pub anchor: ReleaseAnchor,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Phase {
    AwaitSam,
    AwaitGuns {
        after_scan: u64,
        completed_scan: Option<u64>,
    },
    Released {
        at: i64,
    },
    Aborted {
        at: i64,
        reason: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pass {
    pub identity: String,
    pub accepted_at: i64,
    pub prepared: PreparedRelease,
    pub phase: Phase,
    pub awaited_sams: BTreeSet<String>,
    pub pending: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DefenseWait {
    pub phase: Phase,
    pub awaited_sams: BTreeSet<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefenseGate {
    WaitingSam,
    WaitingGuns,
    Ready { epoch: u64 },
    SourcePending,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub scan_epoch: u64,
    pub scan_open: bool,
    pub drafts: BTreeMap<EntityId, Vec<PreparedRelease>>,
    pub collected: BTreeMap<String, PreparedRelease>,
    pub ready_releases: BTreeSet<EntityId>,
    pub cancellations: BTreeSet<EntityId>,
    pub passes: BTreeMap<EntityId, Pass>,
    pub defense: BTreeMap<String, DefenseRecord>,
    pub generic_defense: BTreeMap<EntityId, DefenseWait>,
    pub unresolved_defense: BTreeMap<String, UnresolvedDefense>,
}
fn turn(w: &World) -> i64 {
    w.tick.div_euclid(
        super::tables()["surface_attack"]["turn_seconds"]
            .as_i64()
            .unwrap(),
    )
}
fn runtime(w: &World) -> Option<&State> {
    w.rules.aircombat.as_ref().map(|s| &s.surface)
}
fn take(w: &mut World) -> Option<State> {
    w.rules
        .aircombat
        .as_mut()
        .map(|s| std::mem::take(&mut s.surface))
}
fn put(w: &mut World, s: State) {
    if let Some(a) = w.rules.aircombat.as_mut() {
        a.surface = s
    }
}
fn pending_event(w: &mut World, id: EntityId, rule: &str, value: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "aircombat2026".into(),
            rule: rule.into(),
            side: w.entity(id).map(|e| e.side),
            unit: Some(id),
            contact: None,
            detail: value.to_string(),
        },
        Visibility::Referee,
    )
}
fn queued(p: &Pass) -> bool {
    matches!(p.phase, Phase::AwaitSam | Phase::AwaitGuns { .. })
}
pub(super) fn bombsight(class: &str) -> Result<Bombsight, String> {
    match super::profile(class)?["bombsight"].as_str() {
        Some("Advanced") => Ok(Bombsight::Advanced),
        Some("Computing") => Ok(Bombsight::Computing),
        Some("Ballistic") => Ok(Bombsight::Ballistic),
        Some("Manual") => Ok(Bombsight::Manual),
        _ => Err("9.6 source bombsight missing".into()),
    }
}
fn geometry(w: &World, p: &PreparedRelease) -> Result<ReleaseAnchor, String> {
    let a = w
        .entity(p.actor)
        .ok_or("9.6 own release aircraft missing")?;
    let altitude = a.altitude_m;
    if !altitude.is_finite() || altitude<0. {return Err("9.6 source release altitude invalid".into());}
    let band=crate::rules::operations::altitude_band(altitude);
    let delivery=match p.delivery {Delivery::Level=>"level",Delivery::Glide=>"glide",_=>return Err("9.6 source delivery geometry not bound".into())};
    let release=super::tables()["surface_attack"]["release_distance_nm_by_band"][delivery][band.to_string()]
        .as_f64().ok_or("9.6 source delivery/altitude release geometry not bound; VLow retarded, VeryHigh and High Glide remain separate")?;
    let distance = a.pos.range_to(p.point);
    let tolerance = super::tables()["surface_attack"]["tolerance_nm"]
        .as_f64()
        .unwrap();
    if !a.pos.x.is_finite()
        || !a.pos.y.is_finite()
        || !distance.is_finite()
        || !a.course_deg.is_finite()
        || !a.ordered_course_deg.is_finite()
        || (distance - release).abs() > tolerance
        || release > 0.
            && crate::geom::angle_diff(a.course_deg, a.pos.bearing_to(p.point)).abs() > 1e-9
        || crate::geom::angle_diff(a.course_deg, a.ordered_course_deg).abs() > 1e-9
    {
        return Err("9.6 own aircraft is outside the captured aim's exact release geometry/straight approach".into());
    }
    Ok(ReleaseAnchor {
        at: w.tick,
        position: a.pos,
        course_deg: a.course_deg,
        altitude_m: altitude,
    })
}
fn own_ready(w: &World, p: &PreparedRelease) -> Result<(), String> {
    if !crate::rules::unmanned2026::can_attack(w, p.actor) {
        return Err("9.6 source aircraft lost attack authorization before physical release".into());
    }
    super::surface_attack_gate(w, p.actor)?;
    let a = w
        .entity(p.actor)
        .filter(|a| {
            a.alive && a.kind == PlatformKind::Aircraft && a.side == p.side && a.class == p.class
        })
        .ok_or("9.6 own release aircraft unavailable or changed source binding")?;
    if !ed::platform_weapons_operational(w, p.actor)
        || !ed::weapon_operational(w, p.actor, &p.weapon)?
        || a.ext.get("nuclear.mission_abort") == Some(&1.)
    {
        return Err("9.6 own aircraft/ordnance is disabled".into());
    }
    crate::rules::nuclear2026::release_gate(w, p.actor, &p.weapon)?;
    if !a
        .weapons
        .iter()
        .any(|m| m.weapon == p.weapon && m.mounts > 0 && m.ammo >= p.rounds)
    {
        return Err("9.6 own source bomb stock no longer available".into());
    }
    let f = w
        .rules
        .aviation
        .as_ref()
        .and_then(|s| s.flights.get(&p.actor))
        .ok_or("9.6 explicit cited payload/tank/fuel state required before bomb release")?;
    if !f.payload_fraction.is_finite()
        || f.external_weapon_rounds
            .get(&p.weapon)
            .copied()
            .unwrap_or(0)
            < p.rounds
    {
        return Err(
            "9.6 explicitly declared external bomb inventory/mass required before release".into(),
        );
    }
    Ok(())
}
fn prepare(
    w: &World,
    actor: EntityId,
    contact: ContactId,
    weapon: &str,
    rounds: u16,
    delivery: Delivery,
    aimpoint: &str,
) -> Result<PreparedRelease, String> {
    if !super::active(w) {
        return Err("9.6 source bombing requires explicit v4".into());
    }
    let a = w
        .entity(actor)
        .filter(|e| e.alive && e.kind == PlatformKind::Aircraft)
        .ok_or("9.6 attacking aircraft unavailable")?;
    let fit = &super::tables()["surface_fits"][&a.class];
    if fit.is_null()
        || fit["weapon"] != weapon
        || !matches!(delivery, Delivery::Glide | Delivery::Level)
    {
        return Err("9.6 this exact source fit supports Mk82 glide/level only; unknown load/delivery not inferred".into());
    }
    if rounds == 0 || rounds as u64 > fit["rounds"].as_u64().unwrap() {
        return Err("9.6 positive salvo within published fitted bomb count required".into());
    }
    let c = w.sides[a.side.index()]
        .contacts
        .get(&contact)
        .filter(|c| {
            !c.stale && !c.bearing_only && c.estimate.x.is_finite() && c.estimate.y.is_finite()
        })
        .ok_or("9.6 own current located target required")?;
    let known_kind = crate::rules::detection2026::known_category(w, a.side, c.about)
        .filter(|k| matches!(k, PlatformKind::Ship | PlatformKind::Base))
        .ok_or("9.6 own classified naval/ground contact required")?;
    let (point, aim_source, no_aa_source, native) = if aimpoint.is_empty() {
        let report=w.rules.aaw.as_ref().and_then(|s|s.firecontrol.tracks.values().filter(|t|t.observer==actor&&t.target==c.about).filter_map(|t|t.report.as_ref()).filter(|r|r.observer==actor&&r.target==c.about&&r.at<=w.tick&&w.tick-r.at<=super::tables()["surface_attack"]["native_report_max_age_s"].as_i64().unwrap()&&r.sensor_kind==crate::db::SensorKind::Visual&&r.measured_position.is_some_and(|p|p.x.is_finite()&&p.y.is_finite())&&a.sensors.iter().any(|m|m.sensor==r.sensor&&!crate::sensors::sensor_damaged(a,&m.sensor))&&w.db.sensors.get(&r.sensor).is_some_and(|d|d.kind==crate::db::SensorKind::Visual)).max_by_key(|r|r.at)).ok_or("9.6 ordinary own-contact attack requires this aircraft's actual native measured Visual report; fused side contact supplies no free onboard observation")?;
        let native = NativeAim {
            observer: actor,
            sensor: report.sensor.clone(),
            at: report.at,
            measured_position: report.measured_position.unwrap(),
        };
        (
            native.measured_position,
            "English2026 §9.6.2; this aircraft's captured native Visual measurement".into(),
            None,
            Some(native),
        )
    } else {
        let aim = av::surface_aimpoint(w, a.side, aimpoint)
            .ok_or("9.6 authorized cited known fixed controlled-range aimpoint required")?;
        let no_aa=w.context.aviation_2026.as_ref().and_then(|c|c.no_aa_surface_attack_source.as_deref()).filter(|s|!s.trim().is_empty()).ok_or("9.6 explicit cited scenario NoAA declaration required for the named controlled-range compatibility mode")?;
        if c.estimate.range_to(aim.point)
            > super::tables()["surface_attack"]["tolerance_nm"]
                .as_f64()
                .unwrap()
        {
            return Err(
                "9.6 own contact estimate does not match the declared known range aimpoint".into(),
            );
        }
        (aim.point, aim.source.clone(), Some(no_aa.to_string()), None)
    };
    let mut p = PreparedRelease {
        actor,
        side: a.side,
        class: a.class.clone(),
        contact,
        target: c.about,
        known_kind,
        weapon: weapon.into(),
        rounds,
        delivery,
        aimpoint: aimpoint.into(),
        aim_source,
        point,
        no_aa_source,
        native,
        anchor: ReleaseAnchor {
            at: w.tick,
            position: a.pos,
            course_deg: a.course_deg,
            altitude_m: a.altitude_m,
        },
    };
    own_ready(w, &p)?;
    bombsight(&a.class)?;
    let data = &super::tables()["surface_bombs"][weapon];
    if data.is_null() || !w.db.weapons.contains_key(weapon) {
        return Err("9.6 source bomb definition/row absent".into());
    }
    p.anchor = geometry(w, &p)?;
    Ok(p)
}
/// Same public ABI as the existing direct caller. Ok means declared/queued,
/// never a promise that a live aircraft has already released after defense.
pub(super) fn declare(
    w: &mut World,
    actor: EntityId,
    contact: ContactId,
    weapon: &str,
    rounds: u16,
    delivery: Delivery,
    aimpoint: &str,
) -> Result<(), String> {
    let p = prepare(w, actor, contact, weapon, rounds, delivery, aimpoint)?;
    let a = w
        .rules
        .aircombat
        .as_ref()
        .ok_or("9.6 air-combat source state missing")?;
    if a.surface_last_turn.get(&actor) == Some(&turn(w))
        || a.surface.passes.get(&actor).is_some_and(queued)
    {
        return Err("9.6 one declared bombing pass per aircraft per source Tactical Turn; prior pass still reserved".into());
    }
    let equivalent = serde_json::to_value(&p).unwrap();
    let drafts = &mut w.rules.aircombat.as_mut().unwrap().surface.drafts;
    if !drafts.get(&actor).is_some_and(|v| {
        v.iter()
            .any(|q| serde_json::to_value(q).unwrap() == equivalent)
    }) {
        drafts.entry(actor).or_default().push(p);
    }
    Ok(())
}
fn active_sams(w: &World, s: &State, id: EntityId) -> BTreeSet<String> {
    s.defense
        .iter()
        .filter(|(_, r)| r.aircraft == id)
        .filter_map(|(identity, r)| match &r.kind {
            SurfaceDefenseKind::Sam { projectiles }
                if projectiles.iter().any(|id| {
                    w.entity(*id).is_some_and(|e| {
                        e.alive
                            && e.projectile
                                .as_ref()
                                .is_some_and(|p| p.target == Some(r.aircraft))
                    })
                }) =>
            {
                Some(identity.clone())
            }
            _ => None,
        })
        .collect()
}
/// Source packets are indexed directly, not reconstructed from event tails.
/// This also covers a v4 save made before the new defense-receipt field existed.
/// `shots` aliases packet members; the immutable identity deduplicates them.
fn ingest_live_source_sams(w: &World, s: &mut State) {
    let Some(aaw) = w.rules.aaw.as_ref() else {
        return;
    };
    for packet in aaw.shots.values() {
        if s.defense.contains_key(&packet.identity)
            || !w
                .entity(packet.permit.target)
                .is_some_and(|e| e.kind == PlatformKind::Aircraft)
            || !w
                .db
                .weapons
                .get(&packet.permit.weapon)
                .is_some_and(|d| d.kind == crate::db::WeaponKind::Sam)
            || !packet
                .members
                .iter()
                .any(|id| w.entity(*id).is_some_and(|e| e.alive))
        {
            continue;
        }
        s.defense.insert(
            packet.identity.clone(),
            DefenseRecord {
                at: packet.permit.tick,
                aircraft: packet.permit.target,
                shooter: packet.permit.shooter,
                kind: SurfaceDefenseKind::Sam {
                    projectiles: packet.members.clone(),
                },
            },
        );
    }
}
fn scan_phase(
    phase: &mut Phase,
    awaited: &mut BTreeSet<String>,
    active: BTreeSet<String>,
    epoch: u64,
    end: bool,
) {
    if !matches!(phase, Phase::AwaitSam | Phase::AwaitGuns { .. }) {
        return;
    }
    if !active.is_empty() {
        awaited.extend(active);
        *phase = Phase::AwaitSam;
    } else if !end && matches!(phase, Phase::AwaitSam) {
        *phase = Phase::AwaitGuns {
            after_scan: epoch.saturating_sub(1),
            completed_scan: None,
        };
    } else if end {
        if let Phase::AwaitGuns {
            after_scan,
            completed_scan,
        } = phase
        {
            if epoch > *after_scan {
                *completed_scan = Some(epoch);
            }
        }
    }
}
pub fn request_defense(w: &mut World, actor: EntityId) -> Result<(), String> {
    if !super::active(w)
        || !w
            .entity(actor)
            .is_some_and(|e| e.alive && e.kind == PlatformKind::Aircraft)
    {
        return Err("9.6 generic defense request requires an active actual aircraft".into());
    }
    let Some(mut s) = take(w) else {
        return Err("9.6 source defense state unavailable".into());
    };
    let active = active_sams(w, &s, actor);
    let after = s.scan_epoch.saturating_sub(u64::from(s.scan_open));
    s.generic_defense
        .entry(actor)
        .or_insert_with(|| DefenseWait {
            phase: if active.is_empty() {
                Phase::AwaitGuns {
                    after_scan: after,
                    completed_scan: None,
                }
            } else {
                Phase::AwaitSam
            },
            awaited_sams: active,
        });
    put(w, s);
    Ok(())
}
pub fn finish_defense(w: &mut World, actor: EntityId) {
    if let Some(a) = w.rules.aircombat.as_mut() {
        a.surface.generic_defense.remove(&actor);
    }
}
pub fn defense_gate(w: &World, actor: EntityId) -> DefenseGate {
    let Some(s) = runtime(w) else {
        return DefenseGate::SourcePending;
    };
    let Some(wait) = s.generic_defense.get(&actor) else {
        return DefenseGate::WaitingGuns;
    };
    if s.unresolved_defense.values().any(|r| {
        r.aircraft == actor
            && r.at.div_euclid(
                super::tables()["surface_attack"]["turn_seconds"]
                    .as_i64()
                    .unwrap(),
            ) == turn(w)
    }) {
        return DefenseGate::SourcePending;
    }
    if !active_sams(w, s, actor).is_empty() {
        return DefenseGate::WaitingSam;
    }
    match wait.phase {
        Phase::AwaitGuns {
            completed_scan: Some(epoch),
            ..
        } if epoch == s.scan_epoch && !s.scan_open => DefenseGate::Ready { epoch },
        _ => DefenseGate::WaitingGuns,
    }
}
/// Called only after the real source producer has physically committed.
pub fn note_surface_defense(
    w: &mut World,
    aircraft: EntityId,
    shooter: EntityId,
    identity: &str,
    kind: SurfaceDefenseKind,
) {
    if !super::active(w)
        || identity.is_empty()
        || !w
            .entity(aircraft)
            .is_some_and(|e| e.kind == PlatformKind::Aircraft)
    {
        return;
    }
    let Some(mut s) = take(w) else { return };
    if !s.defense.contains_key(identity) {
        s.defense.insert(
            identity.into(),
            DefenseRecord {
                at: w.tick,
                aircraft,
                shooter,
                kind,
            },
        );
    }
    let active = active_sams(w, &s, aircraft);
    if !active.is_empty() {
        if let Some(p) = s.passes.get_mut(&aircraft).filter(|p| queued(p)) {
            p.awaited_sams.extend(active.iter().cloned());
            p.phase = Phase::AwaitSam;
        }
        if let Some(p) = s.generic_defense.get_mut(&aircraft) {
            p.awaited_sams.extend(active);
            p.phase = Phase::AwaitSam;
        }
    }
    put(w, s)
}
/// Only uncertainty in the source calculation blocks adjudication. Known
/// illegal range/arc, empty stock, director off and reaction timers do not.
pub fn defensive_source_unresolved(reason: &str) -> bool {
    [
        "not source-verified",
        "source-verified ATA/guidance/envelope/fit missing",
        "not source-bound",
        "source fit unavailable",
        "unresolved",
        "unknown",
        "exact source fire-control sensor missing",
        "source director/channel fit missing",
        "source physical director count",
        "source arcs missing",
        "authored/verified land geometry",
        "explicit source local-control mode",
    ]
    .iter()
    .any(|term| reason.contains(term))
}
pub fn note_surface_defense_pending(w: &mut World, aircraft: EntityId, reason: &str) {
    if !super::active(w)
        || !defensive_source_unresolved(reason)
        || !w
            .entity(aircraft)
            .is_some_and(|a| a.kind == PlatformKind::Aircraft)
    {
        return;
    }
    let key = format!("{}:{}:{reason}", aircraft.0, turn(w));
    let now = w.tick;
    if let Some(a) = w.rules.aircombat.as_mut() {
        a.surface
            .unresolved_defense
            .entry(key)
            .or_insert_with(|| UnresolvedDefense {
                at: now,
                aircraft,
                reason: reason.into(),
            });
    }
}
/// First barrier provider: validate and collect, without reserving a prefix.
pub fn collect_surface_declarations(w: &mut World) {
    if !super::active(w) {
        return;
    }
    let Some(mut s) = take(w) else { return };
    let mut drafts = std::mem::take(&mut s.drafts);
    for actor in std::mem::take(&mut s.cancellations) {
        drafts.remove(&actor);
        budget::cancel_actor(w, actor);
        if let Some(pass) = s.passes.get_mut(&actor).filter(|p| queued(p)) {
            pass.phase = Phase::Aborted {
                at: w.tick,
                reason: "own controller cancelled the pending pass".into(),
            };
        }
    }
    // Preserve valid reservations from saved native passes; retire genuinely
    // unavailable old approaches before the new all-kind declaration barrier.
    for p in s.passes.values_mut().filter(|p| queued(p)) {
        if let Err(reason) = own_ready(w, &p.prepared) {
            budget::abort(w, &p.identity);
            p.phase = Phase::Aborted { at: w.tick, reason };
        } else {
            budget::adopt_reserved(
                w,
                budget::Claim {
                    identity: p.identity.clone(),
                    actor: p.prepared.actor,
                    target: p.prepared.target,
                    known_kind: p.prepared.known_kind,
                    kind: budget::AirAttackKind::UnguidedBomb,
                },
                p.accepted_at,
            );
        }
    }
    for (actor, variants) in drafts {
        for p in variants {
            if let Err(reason) = own_ready(w, &p) {
                pending_event(
                    w,
                    actor,
                    "9.6.pass_aborted",
                    json!({"reason":reason,"bombs_released":false}),
                );
                continue;
            }
            let identity = budget::claim_identity(
                budget::AirAttackKind::UnguidedBomb,
                actor,
                w.tick,
                &serde_json::to_value(&p).unwrap(),
            );
            let claim = budget::Claim {
                identity: identity.clone(),
                actor,
                target: p.target,
                known_kind: p.known_kind,
                kind: budget::AirAttackKind::UnguidedBomb,
            };
            match budget::submit(w, claim) {
                Ok(()) => {
                    s.collected.insert(identity, p);
                }
                Err(reason) => pending_event(
                    w,
                    actor,
                    "9.6.pass_aborted",
                    json!({"reason":reason,"bombs_released":false}),
                ),
            }
        }
    }
    put(w, s);
}
fn admit_collected(w: &mut World, s: &mut State) {
    for (identity, p) in std::mem::take(&mut s.collected) {
        let decision = budget::decision(w, &identity);
        if decision != budget::Decision::Reserved {
            pending_event(
                w,
                p.actor,
                "9.6.pass_conflict",
                json!({"reason":"all-kind source declaration barrier rejected this pass","decision":decision}),
            );
            continue;
        }
        if let Err(reason) = own_ready(w, &p) {
            budget::abort(w, &identity);
            pending_event(
                w,
                p.actor,
                "9.6.pass_aborted",
                json!({"reason":reason,"bombs_released":false}),
            );
            continue;
        }
        let sams = active_sams(w, s, p.actor);
        let phase = if sams.is_empty() {
            Phase::AwaitGuns {
                after_scan: s.scan_epoch.saturating_sub(1),
                completed_scan: None,
            }
        } else {
            Phase::AwaitSam
        };
        s.passes.insert(
            p.actor,
            Pass {
                identity,
                accepted_at: w.tick,
                prepared: p,
                phase,
                awaited_sams: sams,
                pending: None,
            },
        );
    }
}
pub(super) fn cancel(w: &mut World, actor: EntityId) {
    if let Some(a) = w.rules.aircombat.as_mut() {
        a.surface.cancellations.insert(actor);
    }
}
/// Wrap the one existing normal fire-control scan. No nested weapon scan.
pub fn begin_surface_defense_scan(w: &mut World) {
    if !super::active(w) {
        return;
    }
    let Some(mut s) = take(w) else { return };
    s.scan_epoch = s.scan_epoch.saturating_add(1);
    s.scan_open = true;
    ingest_live_source_sams(w, &mut s);
    admit_collected(w, &mut s);
    let ids: Vec<_> = s.passes.keys().copied().collect();
    for id in ids {
        let active = active_sams(w, &s, id);
        let p = s.passes.get_mut(&id).unwrap();
        scan_phase(
            &mut p.phase,
            &mut p.awaited_sams,
            active,
            s.scan_epoch,
            false,
        );
    }
    let ids: Vec<_> = s.generic_defense.keys().copied().collect();
    for id in ids {
        let active = active_sams(w, &s, id);
        let p = s.generic_defense.get_mut(&id).unwrap();
        scan_phase(
            &mut p.phase,
            &mut p.awaited_sams,
            active,
            s.scan_epoch,
            false,
        );
    }
    put(w, s)
}
pub fn end_surface_defense_scan(w: &mut World) {
    if !super::active(w) {
        return;
    }
    let Some(mut s) = take(w) else { return };
    if s.scan_open {
        let ids: Vec<_> = s.passes.keys().copied().collect();
        for id in ids {
            let active = active_sams(w, &s, id);
            let p = s.passes.get_mut(&id).unwrap();
            scan_phase(
                &mut p.phase,
                &mut p.awaited_sams,
                active,
                s.scan_epoch,
                true,
            );
        }
        let ids: Vec<_> = s.generic_defense.keys().copied().collect();
        for id in ids {
            let active = active_sams(w, &s, id);
            let p = s.generic_defense.get_mut(&id).unwrap();
            scan_phase(
                &mut p.phase,
                &mut p.awaited_sams,
                active,
                s.scan_epoch,
                true,
            );
        }
    }
    s.scan_open = false;
    put(w, s)
}
fn defense(w: &World, s: &State, p: &Pass) -> Result<(bool, bool), String> {
    defense_for(w, s, p.prepared.actor, &p.awaited_sams)
}
fn defense_for(
    w: &World,
    s: &State,
    actor: EntityId,
    awaited_sams: &BTreeSet<String>,
) -> Result<(bool, bool), String> {
    let source_turn = turn(w);
    let mut any = false;
    let mut heavy = false;
    for (id, r) in &s.defense {
        if r.aircraft != actor
            || !(r.at.div_euclid(
                super::tables()["surface_attack"]["turn_seconds"]
                    .as_i64()
                    .unwrap(),
            ) == source_turn
                || awaited_sams.contains(id))
        {
            continue;
        }
        any = true;
        match &r.kind {
            SurfaceDefenseKind::Gun { source_aa_rating } => {
                if !source_aa_rating.is_finite() || *source_aa_rating < 0. {
                    return Err("9.6 actual AA source rating unresolved".into());
                }
                heavy |= *source_aa_rating
                    >= super::tables()["surface_attack"]["heavy_aa_source_rating"]
                        .as_f64()
                        .unwrap();
            }
            SurfaceDefenseKind::Sam { .. } => heavy = true,
        }
    }
    Ok((!any, heavy))
}
/// Read the same committed source AA ledger used by ordinary bombing.
/// `(no_aa, heavy_aa)` is private calculation input, never target disclosure.
pub fn defense_modifiers(w: &World, actor: EntityId) -> Result<(bool, bool), String> {
    let s = runtime(w).ok_or("9.6 source defense state unavailable")?;
    if s.unresolved_defense.values().any(|r| {
        r.aircraft == actor
            && r.at.div_euclid(
                super::tables()["surface_attack"]["turn_seconds"]
                    .as_i64()
                    .unwrap(),
            ) == turn(w)
    }) {
        return Err("9.6 actual defensive fire source unresolved".into());
    }
    let empty = BTreeSet::new();
    let awaited = s
        .generic_defense
        .get(&actor)
        .map(|p| &p.awaited_sams)
        .unwrap_or(&empty);
    defense_for(w, s, actor, awaited)
}
/// After normal projectile advancement and before common damage finish.
pub fn prepare_surface_releases(w: &mut World) {
    if !super::active(w) {
        return;
    }
    let Some(mut s) = take(w) else { return };
    let ids: Vec<_> = s.passes.keys().copied().collect();
    s.ready_releases.clear();
    for actor in ids {
        let mut p = s.passes.remove(&actor).unwrap();
        if !queued(&p) {
            s.passes.insert(actor, p);
            continue;
        }
        if let Err(reason) = own_ready(w, &p.prepared) {
            budget::abort(w, &p.identity);
            p.phase = Phase::Aborted {
                at: w.tick,
                reason: reason.clone(),
            };
            pending_event(
                w,
                actor,
                "9.6.pass_aborted",
                json!({"identity":p.identity,"reason":reason,"bombs_released":false}),
            );
            s.passes.insert(actor, p);
            continue;
        }
        let active = active_sams(w, &s, actor);
        if !active.is_empty() {
            p.awaited_sams.extend(active);
            p.phase = Phase::AwaitSam;
            s.passes.insert(actor, p);
            continue;
        }
        if matches!(p.phase, Phase::AwaitSam) {
            p.phase = Phase::AwaitGuns {
                after_scan: s.scan_epoch,
                completed_scan: None,
            };
            s.passes.insert(actor, p);
            continue;
        }
        let scanned =
            matches!(p.phase,Phase::AwaitGuns{completed_scan:Some(epoch),..}if epoch==s.scan_epoch);
        if !scanned {
            s.passes.insert(actor, p);
            continue;
        }
        // Only the same1s command-arrival stage can use its captured own anchor.
        // A later tick must satisfy real current position/course against the held aim.
        let own_heading_retained = w.entity(actor).is_some_and(|a| {
            crate::geom::angle_diff(a.course_deg, p.prepared.anchor.course_deg).abs() <= 1e-9
                && crate::geom::angle_diff(a.ordered_course_deg, p.prepared.anchor.course_deg).abs()
                    <= 1e-9
        });
        let command_arrival_stage = p.accepted_at == w.tick
            && p.prepared.anchor.at <= w.tick
            && w.tick - p.prepared.anchor.at <= 1;
        if !command_arrival_stage || !own_heading_retained {
            match geometry(w, &p.prepared) {
                Ok(anchor) => p.prepared.anchor = anchor,
                Err(reason) => {
                    if p.pending.as_deref() != Some(&reason) {
                        pending_event(
                            w,
                            actor,
                            "9.6.pass_geometry_pending",
                            json!({"identity":p.identity,"reason":reason}),
                        );
                        p.pending = Some(reason);
                    }
                    p.phase = Phase::AwaitGuns {
                        after_scan: s.scan_epoch,
                        completed_scan: None,
                    };
                    s.passes.insert(actor, p);
                    continue;
                }
            }
        }
        if s.unresolved_defense.values().any(|r| {
            r.aircraft == actor
                && r.at.div_euclid(
                    super::tables()["surface_attack"]["turn_seconds"]
                        .as_i64()
                        .unwrap(),
                ) == turn(w)
        }) {
            let reason = "source defensive-fire calculation unresolved; not treated as NoAA";
            if p.pending.as_deref() != Some(reason) {
                pending_event(
                    w,
                    actor,
                    "9.6.pass_defense_pending",
                    json!({"identity":p.identity,"reason":reason}),
                );
                p.pending = Some(reason.into());
            }
            p.phase = Phase::AwaitGuns {
                after_scan: s.scan_epoch,
                completed_scan: None,
            };
            s.passes.insert(actor, p);
            continue;
        }
        if budget::decision(w, &p.identity) == budget::Decision::Reserved {
            s.ready_releases.insert(actor);
        }
        s.passes.insert(actor, p);
    }
    // Put back the completed normal scan before the shared service validates
    // its epoch; surface state must never disappear from a sibling provider.
    let offers: Vec<_> = s
        .ready_releases
        .iter()
        .filter_map(|id| s.passes.get(id).map(|p| (p.identity.clone(), s.scan_epoch)))
        .collect();
    put(w, s);
    for (identity, epoch) in offers {
        let _ = budget::offer_ready(w, &identity, epoch);
    }
}
pub fn commit_surface_releases(w: &mut World) {
    if !super::active(w) {
        return;
    }
    let ready: Vec<_> = runtime(w)
        .map(|s| {
            s.ready_releases
                .iter()
                .filter_map(|actor| s.passes.get(actor).map(|p| p.identity.clone()))
                .collect()
        })
        .unwrap_or_default();
    let tickets: BTreeSet<_> = ready
        .into_iter()
        .filter(|identity| budget::take_ticket(w, identity).is_some())
        .collect();
    let Some(mut s) = take(w) else { return };
    for actor in std::mem::take(&mut s.ready_releases) {
        let Some(mut p) = s.passes.remove(&actor) else {
            continue;
        };
        if !tickets.contains(&p.identity) {
            let reason = "simultaneous ready passes exceed the remaining source twelve-aircraft budget; all new releases held";
            if p.pending.as_deref() != Some(reason) {
                pending_event(
                    w,
                    actor,
                    "9.6.pass_capacity_pending",
                    json!({"identity":p.identity,"reason":reason}),
                );
                p.pending = Some(reason.into());
            }
            p.phase = Phase::AwaitGuns {
                after_scan: s.scan_epoch,
                completed_scan: None,
            };
            s.passes.insert(actor, p);
            continue;
        }
        if let Err(reason) = own_ready(w, &p.prepared) {
            budget::abort(w, &p.identity);
            p.phase = Phase::Aborted {
                at: w.tick,
                reason: reason.clone(),
            };
            pending_event(
                w,
                actor,
                "9.6.pass_aborted",
                json!({"identity":p.identity,"reason":reason,"bombs_released":false}),
            );
            s.passes.insert(actor, p);
            continue;
        }
        let conditions = defense(w, &s, &p);
        match conditions
            .and_then(|(no_aa, heavy)| super::execute_surface_release(w, &p.prepared, no_aa, heavy))
        {
            Ok(()) => {
                budget::finish(w, &p.identity);
                p.phase = Phase::Released { at: w.tick };
                p.pending = None;
            }
            Err(reason) => {
                budget::abort(w, &p.identity);
                p.phase = Phase::Aborted {
                    at: w.tick,
                    reason: reason.clone(),
                };
                pending_event(
                    w,
                    actor,
                    "9.6.pass_aborted",
                    json!({"identity":p.identity,"reason":reason,"bombs_released":false}),
                );
            }
        }
        s.passes.insert(actor, p);
    }
    put(w, s)
}
/// Compatibility for explicit single-domain callers; the normal sim uses all
/// providers' prepare functions, one close_ready, then all commit functions.
pub fn resolve_surface_attacks(w: &mut World) {
    prepare_surface_releases(w);
    budget::close_ready(w);
    commit_surface_releases(w);
}

pub(super) fn own_view(w: &World, id: EntityId) -> Option<Value> {
    let p = runtime(w)?.passes.get(&id)?;
    Some(
        json!({"contact":p.prepared.contact,"weapon":p.prepared.weapon,"rounds":p.prepared.rounds,"delivery":p.prepared.delivery,"declared_at":p.accepted_at,"status":match p.phase{Phase::Released{..}=>"released",Phase::Aborted{..}=>"cancelled",_=>"pending"}}),
    )
}

//! Ordinary §9.6.6 cluster bombs; distinct from ASW cluster depth charges.
use crate::db::{Guidance, Targets, WeaponDef, WeaponFit, WeaponKind};
use crate::rules::{aircombat2026 as ac, aviation2026 as av};
use crate::types::PlatformKind;
use crate::{ContactId, Database, EntityId, EventKind, Side, Vec2, Visibility, World};
use ac::{budget, surface, Delivery};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
mod impact;
pub fn tables() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/cluster-2026.json"))
            .expect("ordinary cluster source data")
    })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub enabled_laboratory_fits: BTreeSet<String>,
    pub coverages: BTreeMap<String, Coverage>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    pub source: String,
    pub aimpoint: String,
    pub primary: String,
    pub members: Vec<String>,
    pub geometry: String,
    pub allowed_sides: Vec<Side>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundCoverage {
    pub primary: EntityId,
    pub members: Vec<EntityId>,
    pub point: Vec2,
    pub source: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Configuration,
    pub coverages: BTreeMap<String, BoundCoverage>,
    pub drafts: BTreeMap<EntityId, Vec<Prepared>>,
    pub pending: BTreeMap<EntityId, Prepared>,
    pub outcomes: BTreeMap<String, Value>,
    pub own_status: BTreeMap<EntityId, String>,
    pub rolls: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub contact: ContactId,
    pub weapon: String,
    pub rounds: u16,
    pub delivery: Delivery,
    pub pattern: String,
    pub coverage: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prepared {
    pub identity: String,
    pub accepted_at: i64,
    pub actor: EntityId,
    pub side: Side,
    pub class: String,
    pub request: Request,
    pub target: EntityId,
    pub known_kind: PlatformKind,
    /// Private immutable admission set captured from this side's located,
    /// classified contacts, never the covered entities' true categories.
    #[serde(default)]
    pub planned_ships: BTreeSet<EntityId>,
    /// Old pending saves do not silently gain a new intelligence capture.
    #[serde(default)]
    pub planned_ships_captured: bool,
    pub point: Vec2,
    pub anchor: surface::ReleaseAnchor,
}
fn active(w: &World) -> bool {
    w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full && w.rules.cluster.is_some()
}
fn trace(w: &mut World, actor: EntityId, rule: &str, detail: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "cluster2026".into(),
            rule: rule.into(),
            side: w.entity(actor).map(|e| e.side),
            unit: Some(actor),
            contact: None,
            detail: detail.to_string(),
        },
        Visibility::Referee,
    );
}
pub fn source_weapon(def: &WeaponDef) -> bool {
    tables()["laboratory_weapons"].get(&def.id).is_some()
}
pub fn configure_database(db: &mut Database) {
    for (id, row) in tables()["laboratory_weapons"].as_object().unwrap() {
        db.weapons.insert(
            id.clone(),
            WeaponDef {
                id: id.clone(),
                name: row["name"].as_str().unwrap().into(),
                kind: WeaponKind::Bomb,
                targets: Targets {
                    surface: true,
                    land: true,
                    ..Default::default()
                },
                range_nm: 0.,
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
                rule_data: Some(super::damage_runtime::WeaponDamage {
                    ship_dp: Some(0.),
                    penetration_cm: row["penetration_cm"].as_f64(),
                    profile: "level".into(),
                    bomb_kind: "Cluster".into(),
                    source: row["source"].as_str().unwrap().into(),
                    ..Default::default()
                }),
            },
        );
    }
    for (id, row) in tables()["laboratory_fits"].as_object().unwrap() {
        let Some(mut aircraft) = db.platforms.get(row["base"].as_str().unwrap()).cloned() else {
            continue;
        };
        aircraft.id = id.clone();
        aircraft.name = row["name"].as_str().unwrap().into();
        aircraft.weapons = vec![WeaponFit {
            weapon: row["weapon"].as_str().unwrap().into(),
            mounts: 1,
            ammo: row["rounds"].as_u64().unwrap() as u16,
        }];
        db.platforms.insert(id.clone(), aircraft);
    }
}
fn named(w: &World, name: &str) -> Result<EntityId, String> {
    let matches: Vec<_> = w
        .entities
        .values()
        .filter(|e| e.name == name)
        .map(|e| e.id)
        .collect();
    if matches.len() == 1 {
        Ok(matches[0])
    } else {
        Err(format!(
            "9.6.6 one unique author coverage object required: {name}"
        ))
    }
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    if w.rules.profile != super::damage_runtime::RuleProfile::Rts2026Full {
        return Ok(());
    }
    let config = w.context.cluster_2026.clone().unwrap_or_default();
    if w.context.cluster_2026.is_some() && !config.source.starts_with("scenario_assumption:") {
        return Err("9.6.6 explicit laboratory scenario_assumption source required; no real Annex fit inferred".into());
    }
    for class in &config.enabled_laboratory_fits {
        if tables()["laboratory_fits"][class].is_null() {
            return Err("9.6.6 unregistered laboratory fit".into());
        }
    }
    let mut bound = BTreeMap::new();
    let tolerance = ac::tables()["surface_attack"]["tolerance_nm"]
        .as_f64()
        .unwrap();
    for (key, c) in &config.coverages {
        let unique: BTreeSet<_> = c.members.iter().collect();
        if key.is_empty()
            || !c.source.starts_with("scenario_assumption:")
            || c.geometry != "scenario_declared_static_adjacency"
            || c.allowed_sides.is_empty()
            || unique.len() != c.members.len()
            || !c.members.contains(&c.primary)
        {
            return Err("9.6.6 complete explicit static author adjacency group required; no radius/footprint inferred".into());
        }
        let point = w
            .context
            .aviation_2026
            .as_ref()
            .and_then(|cfg| cfg.surface_aimpoints.get(&c.aimpoint))
            .filter(|a| {
                !a.source.trim().is_empty()
                    && a.controlled_range
                    && a.fixed_target
                    && a.point.x.is_finite()
                    && a.point.y.is_finite()
            })
            .ok_or("9.6.6 coverage requires cited fixed experiment anchor")?
            .point;
        let members: Vec<_> = c
            .members
            .iter()
            .map(|name| named(w, name))
            .collect::<Result<_, _>>()?;
        for id in &members {
            let member = w.entity(*id).unwrap();
            if !member.alive
                || !matches!(member.kind, PlatformKind::Ship | PlatformKind::Base)
                || member.speed_kts != 0.
                || member.pos.range_to(point) > tolerance
            {
                return Err("9.6.6 this narrow author adjacency model requires static source objects at its one abstract anchor; no spread geometry guessed".into());
            }
        }
        bound.insert(
            key.clone(),
            BoundCoverage {
                primary: named(w, &c.primary)?,
                members,
                point,
                source: c.source.clone(),
            },
        );
    }
    for actor in w
        .entities
        .values()
        .filter(|e| tables()["laboratory_fits"].get(&e.class).is_some())
    {
        let row = &tables()["laboratory_fits"][&actor.class];
        let weapon = row["weapon"].as_str().unwrap();
        let rounds = row["rounds"].as_u64().unwrap() as u16;
        let f = w
            .rules
            .aviation
            .as_ref()
            .and_then(|s| s.flights.get(&actor.id))
            .ok_or("9.6.6 source airframe and explicit laboratory payload/fuel state required")?;
        if !config.enabled_laboratory_fits.contains(&actor.class)
            || actor.weapons.len() != 1
            || actor.weapons[0].weapon != weapon
            || actor.weapons[0].mounts != 1
            || actor.weapons[0].ammo != rounds
            || f.external_weapon_rounds.get(weapon) != Some(&rounds)
            || !f.internal_weapon_rounds.is_empty()
            || (f.payload_fraction * f.spec.max_payload_kg - row["payload_kg"].as_f64().unwrap())
                .abs()
                > 1e-8
        {
            return Err(
                "9.6.6 explicit laboratory physical stock/mass differs from registered fit".into(),
            );
        }
        if w.rules.nuclear.as_ref().is_some_and(|s| {
            s.fits
                .get(&actor.id)
                .is_some_and(|m| m.contains_key(weapon))
        }) {
            return Err(
                "9.6.6 laboratory conventional cluster bomb has no nuclear counterpart".into(),
            );
        }
    }
    w.rules.cluster = Some(State {
        config,
        coverages: bound,
        ..Default::default()
    });
    Ok(())
}
fn own_ready(w: &World, p: &Prepared) -> Result<(), String> {
    if !p.planned_ships_captured {
        return Err("9.6.6 planned member classification was not captured; legacy/unknown collateral budget source pending".into());
    }
    ac::surface_attack_gate(w, p.actor)?;
    if !super::unmanned2026::can_attack(w, p.actor)
        || !super::expanded_damage2026::platform_weapons_operational(w, p.actor)
        || !super::expanded_damage2026::weapon_operational(w, p.actor, &p.request.weapon)?
    {
        return Err("9.6.6 own aircraft/weapon attack authorization lost".into());
    }
    let a = w
        .entity(p.actor)
        .filter(|e| {
            e.alive && e.kind == PlatformKind::Aircraft && e.class == p.class && e.side == p.side
        })
        .ok_or("9.6.6 own source aircraft unavailable")?;
    if a.ext.get("nuclear.mission_abort") == Some(&1.)
        || w.rules.nuclear.as_ref().is_some_and(|s| {
            s.fits
                .get(&p.actor)
                .is_some_and(|m| m.contains_key(&p.request.weapon))
        })
    {
        return Err(
            "9.6.6 conventional cluster mission aborted or incompatible nuclear fit".into(),
        );
    }
    super::nuclear2026::release_gate(w, p.actor, &p.request.weapon)?;
    if !a
        .weapons
        .iter()
        .any(|m| m.weapon == p.request.weapon && m.mounts == 1 && m.ammo >= p.request.rounds)
        || w.rules
            .aviation
            .as_ref()
            .and_then(|s| s.flights.get(&p.actor))
            .and_then(|f| f.external_weapon_rounds.get(&p.request.weapon))
            .copied()
            .unwrap_or(0)
            < p.request.rounds
    {
        return Err("9.6.6 physical cluster stock/declared external mass unavailable".into());
    }
    Ok(())
}
fn geometry(w: &World, p: &Prepared) -> Result<surface::ReleaseAnchor, String> {
    let a = w
        .entity(p.actor)
        .ok_or("9.6.6 own release aircraft absent")?;
    let h = a.altitude_m;
    if !h.is_finite()
        || h <= 0.
        || h > 7500.
        || !matches!(p.request.delivery, Delivery::Glide | Delivery::Level)
    {
        return Err("9.6.6 this lab source consumer supports Level/Glide up through Medium only; High/Lob mapping pending".into());
    }
    let release = if p.request.delivery == Delivery::Level && h > 2000. {
        3.
    } else {
        0.
    };
    let d = a.pos.range_to(p.point);
    let tolerance = ac::tables()["surface_attack"]["tolerance_nm"]
        .as_f64()
        .unwrap();
    if !a.pos.x.is_finite()
        || !a.pos.y.is_finite()
        || !a.course_deg.is_finite()
        || !a.ordered_course_deg.is_finite()
        || !d.is_finite()
        || (d - release).abs() > tolerance
        || crate::geom::angle_diff(a.course_deg, a.ordered_course_deg).abs() > 1e-9
        || release > 0.
            && crate::geom::angle_diff(a.course_deg, a.pos.bearing_to(p.point)).abs() > 1e-9
    {
        return Err("9.6.6 own aircraft outside actual straight source release geometry".into());
    }
    Ok(surface::ReleaseAnchor {
        at: w.tick,
        position: a.pos,
        course_deg: a.course_deg,
        altitude_m: h,
    })
}
/// The assigned author group is an allowed binding, not a classification.
/// Only this side's current located contacts and obtained categories can
/// supply the ship budget set. An unclassified member remains pending.
fn capture_planned_ships(
    w: &World,
    side: Side,
    members: &[EntityId],
    point: Vec2,
) -> Result<BTreeSet<EntityId>, String> {
    let tolerance = ac::tables()["surface_attack"]["tolerance_nm"]
        .as_f64()
        .unwrap();
    let mut ships = BTreeSet::new();
    for member in members {
        let contact = w.sides[side.index()]
            .contacts
            .values()
            .filter(|c| {
                c.about == *member
                    && !c.stale
                    && !c.bearing_only
                    && c.last_update_tick <= w.tick
                    && c.quality.is_finite()
                    && c.quality > 0.
                    && c.estimate.x.is_finite()
                    && c.estimate.y.is_finite()
            })
            .max_by_key(|c| (c.last_update_tick, std::cmp::Reverse(c.id)))
            .ok_or("9.6.6 planned coverage member lacks own current located contact; unknown collateral budget source pending")?;
        if contact.estimate.range_to(point) > tolerance {
            return Err("9.6.6 own located member does not match the allocated static coverage anchor; no hidden placement correction".into());
        }
        match super::detection2026::known_category(w, side, contact.about) {
            Some(PlatformKind::Ship) => {
                ships.insert(*member);
            }
            Some(PlatformKind::Base) => {}
            _ => return Err("9.6.6 planned coverage member lacks own obtained naval/ground classification; unknown collateral budget source pending".into()),
        }
    }
    Ok(ships)
}
fn prepare(w: &World, actor: EntityId, request: Request) -> Result<Prepared, String> {
    if !active(w) {
        return Err("9.6.6 ordinary cluster consumer requires explicit v4".into());
    }
    let a = w
        .entity(actor)
        .filter(|e| e.alive && e.kind == PlatformKind::Aircraft)
        .ok_or("9.6.6 own live aircraft required")?;
    let state = w.rules.cluster.as_ref().unwrap();
    let fit = &tables()["laboratory_fits"][&a.class];
    if fit.is_null()
        || !state.config.enabled_laboratory_fits.contains(&a.class)
        || fit["weapon"] != request.weapon
        || request.rounds != 1
        || !matches!(request.pattern.as_str(), "small" | "large")
    {
        return Err("9.6.6 registered lab fit and one-bomb Small/Large source subset required; multi-bomb Salvo priority pending".into());
    }
    let c = w.sides[a.side.index()]
        .contacts
        .get(&request.contact)
        .filter(|c| {
            !c.stale && !c.bearing_only && c.estimate.x.is_finite() && c.estimate.y.is_finite()
        })
        .ok_or("9.6.6 own current located primary contact required")?;
    let known_kind = super::detection2026::known_category(w, a.side, c.about)
        .filter(|k| matches!(k, PlatformKind::Ship | PlatformKind::Base))
        .ok_or("9.6.6 own classified naval/ground primary contact required")?;
    let cfg = state
        .config
        .coverages
        .get(&request.coverage)
        .filter(|g| g.allowed_sides.contains(&a.side))
        .ok_or("9.6.6 allocated author coverage not available to own side")?;
    let group = state
        .coverages
        .get(&request.coverage)
        .filter(|g| g.primary == c.about)
        .ok_or("9.6.6 primary contact differs from assigned coverage target")?;
    let aim = av::surface_aimpoint(w, a.side, &cfg.aimpoint)
        .ok_or("9.6.6 own allocated fixed coverage anchor unavailable")?;
    if aim.point.range_to(group.point) > 1e-9
        || c.estimate.range_to(group.point)
            > ac::tables()["surface_attack"]["tolerance_nm"]
                .as_f64()
                .unwrap()
    {
        return Err(
            "9.6.6 own located primary does not match allocated static adjacency anchor".into(),
        );
    }
    let planned_ships = capture_planned_ships(w, a.side, &group.members, group.point)?;
    let mut p = Prepared {
        identity: String::new(),
        accepted_at: w.tick,
        actor,
        side: a.side,
        class: a.class.clone(),
        request,
        target: c.about,
        known_kind,
        planned_ships,
        planned_ships_captured: true,
        point: group.point,
        anchor: surface::ReleaseAnchor {
            at: w.tick,
            position: a.pos,
            course_deg: a.course_deg,
            altitude_m: a.altitude_m,
        },
    };
    own_ready(w, &p)?;
    p.anchor = geometry(w, &p)?;
    p.identity = budget::claim_identity(
        budget::AirAttackKind::UnguidedBomb,
        actor,
        p.accepted_at,
        &serde_json::to_value(&p.request).unwrap(),
    );
    Ok(p)
}
pub fn declare(
    w: &mut World,
    actor: EntityId,
    contact: ContactId,
    weapon: &str,
    rounds: u16,
    delivery: Delivery,
    pattern: &str,
    coverage: &str,
) -> Result<(), String> {
    let p = prepare(
        w,
        actor,
        Request {
            contact,
            weapon: weapon.into(),
            rounds,
            delivery,
            pattern: pattern.into(),
            coverage: coverage.into(),
        },
    )?;
    let s = w.rules.cluster.as_mut().unwrap();
    if s.pending.contains_key(&actor) {
        return Err("9.6.6 another cluster pass is pending".into());
    }
    let list = s.drafts.entry(actor).or_default();
    if !list.iter().any(|old| old.request == p.request) {
        list.push(p);
    }
    Ok(())
}
fn abort(w: &mut World, p: &Prepared, reason: String) {
    budget::abort(w, &p.identity);
    surface::finish_defense(w, p.actor);
    w.rules.cluster.as_mut().unwrap().pending.remove(&p.actor);
    w.rules
        .cluster
        .as_mut()
        .unwrap()
        .own_status
        .insert(p.actor, "cancelled".into());
    trace(
        w,
        p.actor,
        "9.6.6.release_aborted",
        json!({"identity":p.identity,"reason":reason,"consumed":0}),
    );
}
pub fn before_defense_scan(w: &mut World) {
    if !active(w) {
        return;
    }
    let drafts = std::mem::take(&mut w.rules.cluster.as_mut().unwrap().drafts);
    for (actor, mut variants) in drafts {
        if variants.len() != 1 {
            budget::cancel_actor(w, actor);
            trace(w, actor, "9.6.6.release_conflict", json!({"consumed":0}));
            continue;
        }
        let p = variants.pop().unwrap();
        if let Err(reason) = own_ready(w, &p).and_then(|_| surface::request_defense(w, actor)) {
            abort(w, &p, reason);
            continue;
        }
        w.rules.cluster.as_mut().unwrap().pending.insert(actor, p);
        w.rules
            .cluster
            .as_mut()
            .unwrap()
            .own_status
            .insert(actor, "pending".into());
    }
    let pending: Vec<_> = w
        .rules
        .cluster
        .as_ref()
        .unwrap()
        .pending
        .values()
        .cloned()
        .collect();
    for p in pending {
        if !p.planned_ships_captured {
            abort(w, &p, "9.6.6 planned member classification was not captured; legacy/unknown collateral budget source pending".into());
            continue;
        }
        if let Err(reason) = budget::submit_planned(
            w,
            budget::Claim {
                identity: p.identity.clone(),
                actor: p.actor,
                target: p.target,
                known_kind: p.known_kind,
                kind: budget::AirAttackKind::UnguidedBomb,
            },
            p.planned_ships.clone(),
        ) {
            abort(w, &p, reason);
        }
    }
}
fn current_anchor(w: &World, p: &Prepared) -> Result<surface::ReleaseAnchor, String> {
    let straight = w.entity(p.actor).is_some_and(|e| {
        crate::geom::angle_diff(e.course_deg, p.anchor.course_deg).abs() <= 1e-9
            && crate::geom::angle_diff(e.ordered_course_deg, p.anchor.course_deg).abs() <= 1e-9
    });
    if p.accepted_at == w.tick && p.anchor.at <= w.tick && w.tick - p.anchor.at <= 1 && straight {
        Ok(p.anchor.clone())
    } else {
        geometry(w, p)
    }
}
pub fn prepare_releases(w: &mut World) {
    if !active(w) {
        return;
    }
    let pending: Vec<_> = w
        .rules
        .cluster
        .as_ref()
        .unwrap()
        .pending
        .values()
        .cloned()
        .collect();
    for mut p in pending {
        if budget::decision(w, &p.identity) != budget::Decision::Reserved {
            abort(
                w,
                &p,
                format!(
                    "9.6 common aircraft budget {:?}",
                    budget::decision(w, &p.identity)
                ),
            );
            continue;
        }
        if let Err(reason) = own_ready(w, &p) {
            abort(w, &p, reason);
            continue;
        }
        let surface::DefenseGate::Ready { epoch } = surface::defense_gate(w, p.actor) else {
            continue;
        };
        p.anchor = match current_anchor(w, &p) {
            Ok(anchor) => anchor,
            Err(reason) => {
                abort(w, &p, reason);
                continue;
            }
        };
        if let Err(reason) = surface::defense_modifiers(w, p.actor) {
            trace(
                w,
                p.actor,
                "9.6.6.source_pending",
                json!({"identity":p.identity,"reason":reason,"consumed":0}),
            );
            continue;
        }
        w.rules
            .cluster
            .as_mut()
            .unwrap()
            .pending
            .insert(p.actor, p.clone());
        if let Err(reason) = budget::offer_ready(w, &p.identity, epoch) {
            abort(w, &p, reason);
        }
    }
}
pub fn commit_releases(w: &mut World) {
    if !active(w) {
        return;
    }
    let pending: Vec<_> = w
        .rules
        .cluster
        .as_ref()
        .unwrap()
        .pending
        .values()
        .cloned()
        .collect();
    for mut p in pending {
        let Some(ticket) = budget::take_ticket(w, &p.identity) else {
            continue;
        };
        if ticket.actor != p.actor
            || ticket.target != p.target
            || !matches!(surface::defense_gate(w,p.actor),surface::DefenseGate::Ready{epoch}if epoch==ticket.epoch)
        {
            abort(w, &p, "9.6.6 current defense/budget ticket changed".into());
            continue;
        }
        if let Err(reason) = own_ready(w, &p) {
            abort(w, &p, reason);
            continue;
        }
        p.anchor = match current_anchor(w, &p) {
            Ok(anchor) => anchor,
            Err(reason) => {
                abort(w, &p, reason);
                continue;
            }
        };
        let (no_aa, heavy_aa) = match surface::defense_modifiers(w, p.actor) {
            Ok(v) => v,
            Err(reason) => {
                abort(w, &p, reason);
                continue;
            }
        };
        let a = w.entity(p.actor).unwrap().clone();
        let slot = a
            .weapons
            .iter()
            .position(|m| {
                m.weapon == p.request.weapon && m.mounts == 1 && m.ammo >= p.request.rounds
            })
            .unwrap();
        let def = w.db.weapons[&p.request.weapon].clone();
        w.entity_mut(p.actor).unwrap().weapons[slot].ammo -= p.request.rounds;
        av::ordnance_released(w, p.actor, &p.request.weapon, p.request.rounds);
        if p.request.delivery == Delivery::Level {
            let until = w.tick
                + ac::tables()["surface_attack"]["drop_increment_s"]
                    .as_i64()
                    .unwrap();
            w.rules
                .aircombat
                .as_mut()
                .unwrap()
                .level_release_until
                .insert(p.actor, until);
        }
        w.emit(
            EventKind::WeaponsRelease {
                side: p.side,
                shooter: p.actor,
                shooter_name: a.name,
                weapon: def.name,
                count: p.request.rounds as u8,
            },
            Visibility::Side(p.side),
        );
        trace(
            w,
            p.actor,
            "9.6.6.cluster_released",
            json!({"identity":p.identity,"rounds":p.request.rounds,"pattern":p.request.pattern,"anchor":p.anchor,"equipment_source":"scenario_laboratory"}),
        );
        if let Err(reason) = impact::resolve(w, &p, no_aa, heavy_aa) {
            let result = json!({"identity":p.identity,"reason":reason,"physical_release_already_committed":true,"equipment_source":"scenario_laboratory"});
            w.rules
                .cluster
                .as_mut()
                .unwrap()
                .outcomes
                .insert(p.identity.clone(), result.clone());
            trace(w, p.actor, "9.6.6.source_pending", result);
        }
        w.rules.cluster.as_mut().unwrap().pending.remove(&p.actor);
        w.rules
            .cluster
            .as_mut()
            .unwrap()
            .own_status
            .insert(p.actor, "released".into());
        surface::finish_defense(w, p.actor);
        budget::finish(w, &p.identity);
    }
}
pub fn own_view(w: &World, actor: EntityId) -> Option<Value> {
    if !active(w) {
        return None;
    }
    let a = w.entity(actor)?;
    let s = w.rules.cluster.as_ref()?;
    if !s.config.enabled_laboratory_fits.contains(&a.class) {
        return None;
    }
    let coverage: Vec<_> = s
        .config
        .coverages
        .iter()
        .filter(|(_, c)| {
            c.allowed_sides.contains(&a.side)
                && av::surface_aimpoint(w, a.side, &c.aimpoint).is_some()
        })
        .map(|(key, _)| key)
        .collect();
    Some(
        json!({"fit":a.class,"equipment_source":"scenario_laboratory","coverages":coverage,"status":s.own_status.get(&actor),"pending":s.pending.get(&actor).map(|p|json!({"weapon":p.request.weapon,"rounds":p.request.rounds,"pattern":p.request.pattern,"coverage":p.request.coverage,"declared_at":p.accepted_at}))}),
    )
}

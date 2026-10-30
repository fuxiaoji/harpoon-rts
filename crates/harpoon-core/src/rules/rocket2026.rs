//! §9.6.7 actual release and source rocket table. Laboratory fits stay explicit.
use crate::db::{Guidance, Targets, WeaponDef, WeaponFit, WeaponKind};
use crate::rules::{aircombat2026 as ac, aviation2026 as av};
use crate::types::PlatformKind;
use crate::{ContactId, Database, EntityId, EventKind, Side, Vec2, Visibility, World};
use ac::{budget, surface};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
mod impact;

pub fn tables() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/rocket-2026.json"))
            .expect("rocket source data")
    })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub enabled_laboratory_fits: BTreeSet<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Configuration,
    pub drafts: BTreeMap<EntityId, Vec<Prepared>>,
    pub pending: BTreeMap<EntityId, Prepared>,
    pub outcomes: BTreeMap<String, Value>,
    pub own_status: BTreeMap<EntityId, String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub contact: ContactId,
    pub weapon: String,
    pub rounds: u16,
    pub aimpoint: String,
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
    pub point: Vec2,
    pub aim_source: String,
    pub native: Option<surface::NativeAim>,
    pub anchor: surface::ReleaseAnchor,
}
fn active(w: &World) -> bool {
    w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full && w.rules.rocket.is_some()
}
fn trace(w: &mut World, actor: EntityId, rule: &str, detail: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "rocket2026".into(),
            rule: rule.into(),
            side: w.entity(actor).map(|e| e.side),
            unit: Some(actor),
            contact: None,
            detail: detail.to_string(),
        },
        Visibility::Referee,
    );
}
/// This atomic source-table consumer never uses generic projectile launch.
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
                kind: WeaponKind::Rocket,
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
                    ship_dp: row["damage_points"].as_f64(),
                    penetration_cm: row["penetration_cm"].as_f64(),
                    profile: "dive".into(),
                    bomb_kind: "LDGP".into(),
                    source: row["source"].as_str().unwrap().into(),
                    ..Default::default()
                }),
            },
        );
    }
    for (id, row) in tables()["laboratory_fits"].as_object().unwrap() {
        let Some(mut plane) = db.platforms.get(row["base"].as_str().unwrap()).cloned() else {
            continue;
        };
        plane.id = id.clone();
        plane.name = row["name"].as_str().unwrap().into();
        plane.weapons = vec![WeaponFit {
            weapon: row["weapon"].as_str().unwrap().into(),
            mounts: 1,
            ammo: row["rounds"].as_u64().unwrap() as u16,
        }];
        db.platforms.insert(id.clone(), plane);
    }
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    if w.rules.profile != super::damage_runtime::RuleProfile::Rts2026Full {
        return Ok(());
    }
    let config = w.context.rocket_2026.clone().unwrap_or_default();
    if w.context.rocket_2026.is_some() && !config.source.starts_with("scenario_assumption:") {
        return Err("9.6.7 laboratory equipment requires explicit scenario_assumption source; real Annex fit is not inferred".into());
    }
    for class in &config.enabled_laboratory_fits {
        if tables()["laboratory_fits"][class].is_null() {
            return Err("9.6.7 unregistered laboratory fit".into());
        }
    }
    for a in w
        .entities
        .values()
        .filter(|a| tables()["laboratory_fits"].get(&a.class).is_some())
    {
        let row = &tables()["laboratory_fits"][&a.class];
        let weapon = row["weapon"].as_str().unwrap();
        let count = row["rounds"].as_u64().unwrap() as u16;
        if !config.enabled_laboratory_fits.contains(&a.class) {
            return Err("9.6.7 laboratory fit not explicitly enabled".into());
        }
        let flight = w
            .rules
            .aviation
            .as_ref()
            .and_then(|s| s.flights.get(&a.id))
            .ok_or("9.6.7 source airframe and explicitly authored payload/fuel required")?;
        if a.weapons.len() != 1
            || a.weapons[0].weapon != weapon
            || a.weapons[0].mounts != 1
            || a.weapons[0].ammo != count
            || flight.external_weapon_rounds.get(weapon) != Some(&count)
            || !flight.internal_weapon_rounds.is_empty()
            || (flight.payload_fraction * flight.spec.max_payload_kg
                - row["payload_kg"].as_f64().unwrap())
            .abs()
                > 1e-8
        {
            return Err("9.6.7 physical laboratory stock/payload differs from explicit fit".into());
        }
        super::nuclear2026::release_gate(w, a.id, weapon)?;
        if w.rules
            .nuclear
            .as_ref()
            .is_some_and(|s| s.fits.get(&a.id).is_some_and(|m| m.contains_key(weapon)))
        {
            return Err("9.6.7 conventional laboratory rockets have no nuclear counterpart".into());
        }
    }
    w.rules.rocket = Some(State {
        config,
        ..Default::default()
    });
    Ok(())
}
fn geometry(w: &World, p: &Prepared) -> Result<surface::ReleaseAnchor, String> {
    let a = w
        .entity(p.actor)
        .ok_or("9.6.7 own release aircraft absent")?;
    let altitude = a.altitude_m;
    let tolerance = ac::tables()["surface_attack"]["tolerance_nm"]
        .as_f64()
        .unwrap();
    if !altitude.is_finite()
        || altitude <= tables()["altitude_m"]["exclusive_min"].as_f64().unwrap()
        || altitude > tables()["altitude_m"]["inclusive_max"].as_f64().unwrap()
        || !a.pos.x.is_finite()
        || !a.pos.y.is_finite()
        || !a.course_deg.is_finite()
        || !a.ordered_course_deg.is_finite()
        || !a.pos.range_to(p.point).is_finite()
        || a.pos.range_to(p.point) > tolerance
        || crate::geom::angle_diff(a.course_deg, a.ordered_course_deg).abs() > 1e-9
    {
        return Err(
            "9.6.7 rockets require direct overflight at Low/VLow on a straight actual approach"
                .into(),
        );
    }
    Ok(surface::ReleaseAnchor {
        at: w.tick,
        position: a.pos,
        course_deg: a.course_deg,
        altitude_m: altitude,
    })
}
fn own_ready(w: &World, p: &Prepared) -> Result<(), String> {
    ac::surface_attack_gate(w, p.actor)?;
    if !super::unmanned2026::can_attack(w, p.actor)
        || !super::expanded_damage2026::platform_weapons_operational(w, p.actor)
        || !super::expanded_damage2026::weapon_operational(w, p.actor, &p.request.weapon)?
    {
        return Err("9.6.7 own release/weapon authorization lost".into());
    }
    let a = w
        .entity(p.actor)
        .filter(|a| {
            a.alive && a.kind == PlatformKind::Aircraft && a.side == p.side && a.class == p.class
        })
        .ok_or("9.6.7 own source airframe unavailable")?;
    if a.ext.get("nuclear.mission_abort") == Some(&1.) {
        return Err("9.6.7 mission aborted".into());
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
        return Err("9.6.7 physical rocket stock or declared external mass unavailable".into());
    }
    Ok(())
}
fn prepare(w: &World, actor: EntityId, request: Request) -> Result<Prepared, String> {
    if !active(w) {
        return Err("9.6.7 source consumer requires v4".into());
    }
    let a = w
        .entity(actor)
        .filter(|a| a.alive && a.kind == PlatformKind::Aircraft)
        .ok_or("9.6.7 live own aircraft required")?;
    let row = &tables()["laboratory_fits"][&a.class];
    if row.is_null()
        || !w
            .rules
            .rocket
            .as_ref()
            .unwrap()
            .config
            .enabled_laboratory_fits
            .contains(&a.class)
        || row["weapon"] != request.weapon
        || request.rounds == 0
        || request.rounds % 2 != 0
        || u64::from(request.rounds) > row["rounds"].as_u64().unwrap()
    {
        return Err("9.6.7 explicit registered laboratory fit and positive even physical rocket count required".into());
    }
    let c = w.sides[a.side.index()]
        .contacts
        .get(&request.contact)
        .filter(|c| {
            !c.stale && !c.bearing_only && c.estimate.x.is_finite() && c.estimate.y.is_finite()
        })
        .ok_or("9.6.7 current own located contact required")?;
    let known_kind = super::detection2026::known_category(w, a.side, c.about)
        .filter(|k| matches!(k, PlatformKind::Ship | PlatformKind::Base))
        .ok_or("9.6.7 own known naval/ground category required")?;
    let (point, aim_source, native) = if request.aimpoint.is_empty() {
        let report=w.rules.aaw.as_ref().and_then(|s|s.firecontrol.tracks.values().filter(|t|t.observer==actor&&t.target==c.about).filter_map(|t|t.report.as_ref()).filter(|r|r.observer==actor&&r.target==c.about&&r.at<=w.tick&&w.tick-r.at<=1&&r.sensor_kind==crate::db::SensorKind::Visual&&r.measured_position.is_some_and(|p|p.x.is_finite()&&p.y.is_finite())&&a.sensors.iter().any(|m|m.sensor==r.sensor&&!crate::sensors::sensor_damaged(a,&m.sensor))).max_by_key(|r|r.at)).ok_or("9.6.7 native onboard Visual measured target required; a fused contact is no free observation")?;
        let point = report.measured_position.unwrap();
        (
            point,
            "English2026 9.6.2 actual own measured Visual target".into(),
            Some(surface::NativeAim {
                observer: actor,
                sensor: report.sensor.clone(),
                at: report.at,
                measured_position: point,
            }),
        )
    } else {
        let aim = av::surface_aimpoint(w, a.side, &request.aimpoint)
            .ok_or("9.6.7 allocated controlled fixed aimpoint required")?;
        if c.estimate.range_to(aim.point)
            > ac::tables()["surface_attack"]["tolerance_nm"]
                .as_f64()
                .unwrap()
        {
            return Err("9.6.7 own contact differs from allocated aimpoint".into());
        }
        (aim.point, aim.source.clone(), None)
    };
    let mut p = Prepared {
        identity: String::new(),
        accepted_at: w.tick,
        actor,
        side: a.side,
        class: a.class.clone(),
        request,
        target: c.about,
        known_kind,
        point,
        aim_source,
        native,
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
        budget::AirAttackKind::Rocket,
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
    aimpoint: &str,
) -> Result<(), String> {
    let p = prepare(
        w,
        actor,
        Request {
            contact,
            weapon: weapon.into(),
            rounds,
            aimpoint: aimpoint.into(),
        },
    )?;
    let state = w.rules.rocket.as_mut().unwrap();
    if state.pending.contains_key(&actor) {
        return Err("9.6.7 another rocket pass is already pending".into());
    }
    let list = state.drafts.entry(actor).or_default();
    if !list.iter().any(|old| old.request == p.request) {
        list.push(p);
    }
    Ok(())
}
fn abort(w: &mut World, p: &Prepared, reason: String) {
    budget::abort(w, &p.identity);
    surface::finish_defense(w, p.actor);
    w.rules.rocket.as_mut().unwrap().pending.remove(&p.actor);
    w.rules
        .rocket
        .as_mut()
        .unwrap()
        .own_status
        .insert(p.actor, "cancelled".into());
    trace(
        w,
        p.actor,
        "9.6.7.release_aborted",
        json!({"identity":p.identity,"reason":reason,"consumed":0}),
    );
}
pub fn before_defense_scan(w: &mut World) {
    if !active(w) {
        return;
    }
    let drafts = std::mem::take(&mut w.rules.rocket.as_mut().unwrap().drafts);
    for (actor, mut list) in drafts {
        if list.len() != 1 {
            budget::cancel_actor(w, actor);
            trace(w, actor, "9.6.7.release_conflict", json!({"consumed":0}));
            continue;
        }
        let p = list.pop().unwrap();
        if let Err(reason) = own_ready(w, &p).and_then(|_| surface::request_defense(w, actor)) {
            abort(w, &p, reason);
            continue;
        }
        w.rules.rocket.as_mut().unwrap().pending.insert(actor, p);
        w.rules
            .rocket
            .as_mut()
            .unwrap()
            .own_status
            .insert(actor, "pending".into());
    }
    let pending: Vec<_> = w
        .rules
        .rocket
        .as_ref()
        .unwrap()
        .pending
        .values()
        .cloned()
        .collect();
    for p in pending {
        let claim = budget::Claim {
            identity: p.identity.clone(),
            actor: p.actor,
            target: p.target,
            known_kind: p.known_kind,
            kind: budget::AirAttackKind::Rocket,
        };
        if let Err(reason) = budget::submit(w, claim) {
            abort(w, &p, reason);
        }
    }
}
fn current_anchor(w: &World, p: &Prepared) -> Result<surface::ReleaseAnchor, String> {
    let retained = w.entity(p.actor).is_some_and(|a| {
        crate::geom::angle_diff(a.course_deg, p.anchor.course_deg).abs() <= 1e-9
            && crate::geom::angle_diff(a.ordered_course_deg, p.anchor.course_deg).abs() <= 1e-9
    });
    if p.accepted_at == w.tick && p.anchor.at <= w.tick && w.tick - p.anchor.at <= 1 && retained {
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
        .rocket
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
                "9.6.7.source_pending",
                json!({"identity":p.identity,"reason":reason,"consumed":0}),
            );
            continue;
        }
        w.rules
            .rocket
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
        .rocket
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
            abort(w, &p, "9.6.7 actual defense ticket changed".into());
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
            Ok(modifiers) => modifiers,
            Err(reason) => {
                abort(w, &p, reason);
                continue;
            }
        };
        // All admission uses own information. Only after this actual release
        // may the referee inspect true size/armor/movement and derive damage.
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
        w.emit(
            EventKind::WeaponsRelease {
                side: p.side,
                shooter: p.actor,
                shooter_name: a.name,
                weapon: def.name.clone(),
                count: p.request.rounds as u8,
            },
            Visibility::Side(p.side),
        );
        trace(
            w,
            p.actor,
            "9.6.7.rockets_released",
            json!({"identity":p.identity,"rounds":p.request.rounds,"anchor":p.anchor,"equipment_source":"scenario_laboratory"}),
        );
        if let Err(reason) = impact::resolve(w, &p, no_aa, heavy_aa) {
            let result = json!({"identity":p.identity,"reason":reason,"rounds":p.request.rounds,"physical_release_already_committed":true,"equipment_source":"scenario_laboratory"});
            w.rules
                .rocket
                .as_mut()
                .unwrap()
                .outcomes
                .insert(p.identity.clone(), result.clone());
            trace(w, p.actor, "9.6.7.source_pending", result);
        }
        w.rules.rocket.as_mut().unwrap().pending.remove(&p.actor);
        w.rules
            .rocket
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
    let state = w.rules.rocket.as_ref()?;
    if !state.config.enabled_laboratory_fits.contains(&a.class) {
        return None;
    }
    Some(
        json!({"fit":a.class,"equipment_source":"scenario_laboratory","status":state.own_status.get(&actor),"pending":state.pending.get(&actor).map(|p|json!({"weapon":p.request.weapon,"rounds":p.request.rounds,"declared_at":p.accepted_at}))}),
    )
}

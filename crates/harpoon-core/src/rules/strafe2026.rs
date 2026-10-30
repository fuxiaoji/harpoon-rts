//! §9.6.9 integral aircraft guns, source Salvo1 without fictitious shell stock.
use crate::db::{Guidance, Targets, WeaponDef, WeaponKind};
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
        serde_json::from_str(include_str!("../../../../data/rules/strafe-2026.json"))
            .expect("source aircraft strafing table")
    })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub enabled_aircraft: BTreeSet<String>,
    pub ammunition_model: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Configuration,
    pub drafts: BTreeMap<EntityId, Vec<Prepared>>,
    pub pending: BTreeMap<EntityId, Prepared>,
    pub fired_passes: BTreeMap<EntityId, u64>,
    pub own_status: BTreeMap<EntityId, String>,
    pub outcomes: BTreeMap<String, Value>,
    pub rolls: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub contact: ContactId,
    pub delivery: Delivery,
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
    pub gun: String,
    pub gun_rating: f64,
    pub anchor: surface::ReleaseAnchor,
}
fn active(w: &World) -> bool {
    w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full && w.rules.strafe.is_some()
}
fn trace(w: &mut World, actor: EntityId, rule: &str, detail: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "strafe2026".into(),
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
    tables()["guns"].get(&def.id).is_some()
}
pub fn configure_database(db: &mut Database) {
    for (id, row) in tables()["guns"].as_object().unwrap() {
        db.weapons.insert(
            id.clone(),
            WeaponDef {
                id: id.clone(),
                name: row["name"].as_str().unwrap().into(),
                kind: WeaponKind::Gun,
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
                    sub_dp: Some(0.),
                    diameter_mm: row["caliber_mm"].as_f64().unwrap(),
                    profile: "strafe".into(),
                    source: row["source"].as_str().unwrap().into(),
                    ..Default::default()
                }),
            },
        );
    }
    // Source integral guns are profiles, not fabricated finite ammo mounts.
    // Aircraft weapon fits and all existing stores remain exactly as loaded.
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    if w.rules.profile != super::damage_runtime::RuleProfile::Rts2026Full {
        return Ok(());
    }
    let config = w.context.strafe_2026.clone().unwrap_or_default();
    if w.context.strafe_2026.is_some()
        && (config.source.trim().is_empty()
            || config.ammunition_model != tables()["ammunition_model"].as_str().unwrap())
    {
        return Err("9.6.9 explicit cited source and book_abstract_gun_rating model required; no finite shell inventory inferred".into());
    }
    for class in &config.enabled_aircraft {
        if tables()["aircraft"][class].is_null() {
            return Err(
                "9.6.9 caliber/integral gun source not verified for enabled aircraft".into(),
            );
        }
    }
    w.rules.strafe = Some(State {
        config,
        ..Default::default()
    });
    Ok(())
}
fn gun_definition(w: &World, p: &Prepared) -> Result<WeaponDef, String> {
    let profile = &tables()["aircraft"][&p.class];
    let row = &tables()["guns"][&p.gun];
    let def =
        w.db.weapons
            .get(&p.gun)
            .filter(|d| d.kind == WeaponKind::Gun && source_weapon(d))
            .ok_or("9.6.9 actual registered source gun definition missing")?;
    let data = def
        .rule_data
        .as_ref()
        .ok_or("9.6.9 actual source gun caliber metadata missing")?;
    if profile["gun"] != p.gun
        || profile["gun_rating"].as_f64() != Some(p.gun_rating)
        || data.diameter_mm != row["caliber_mm"].as_f64().unwrap()
        || data.source.trim().is_empty()
        || data.penetration_cm.is_some()
        || def.damage != 0.
    {
        return Err(
            "9.6.9 source gun profile changed or acquired unsourced penetration/damage".into(),
        );
    }
    Ok(def.clone())
}
fn own_ready(w: &World, p: &Prepared) -> Result<(), String> {
    ac::surface_attack_gate(w, p.actor)?;
    let a = w
        .entity(p.actor)
        .filter(|e| {
            e.alive && e.kind == PlatformKind::Aircraft && e.side == p.side && e.class == p.class
        })
        .ok_or("9.6.9 own source aircraft unavailable")?;
    if !w
        .rules
        .strafe
        .as_ref()
        .unwrap()
        .config
        .enabled_aircraft
        .contains(&a.class)
        || !p.gun_rating.is_finite()
        || p.gun_rating <= 0.
        || !super::unmanned2026::can_attack(w, p.actor)
        || !super::expanded_damage2026::platform_weapons_operational(w, p.actor)
        || !super::expanded_damage2026::weapon_operational(w, p.actor, &p.gun)?
    {
        return Err("9.6.9 own integral gun/aircraft/crew cannot perform source pass".into());
    }
    if a.ext.get("nuclear.mission_abort") == Some(&1.)
        || w.rules
            .nuclear
            .as_ref()
            .is_some_and(|s| s.fits.get(&p.actor).is_some_and(|m| m.contains_key(&p.gun)))
    {
        return Err(
            "9.6.9 conventional gun pass incompatible with mission abort/nuclear fit".into(),
        );
    }
    super::nuclear2026::release_gate(w, p.actor, &p.gun)?;
    gun_definition(w, p)?;
    if w.rules
        .aviation
        .as_ref()
        .and_then(|s| s.flights.get(&p.actor))
        .is_none()
    {
        return Err("9.6.9 actual own flight/fuel/load source state required".into());
    }
    Ok(())
}
fn geometry(w: &World, p: &Prepared) -> Result<surface::ReleaseAnchor, String> {
    let a = w.entity(p.actor).ok_or("9.6.9 own aircraft missing")?;
    let distance = a.pos.range_to(p.point);
    if !a.altitude_m.is_finite()
        || a.altitude_m <= 0.
        || a.altitude_m > tables()["max_attack_altitude_m"].as_f64().unwrap()
        || !a.pos.x.is_finite()
        || !a.pos.y.is_finite()
        || !distance.is_finite()
        || distance
            > ac::tables()["surface_attack"]["tolerance_nm"]
                .as_f64()
                .unwrap()
        || !a.course_deg.is_finite()
        || !a.ordered_course_deg.is_finite()
        || crate::geom::angle_diff(a.course_deg, a.ordered_course_deg).abs() > 1e-9
    {
        return Err("9.6.9 source strafing needs direct overflight on a straight actual approach at Low or less".into());
    }
    Ok(surface::ReleaseAnchor {
        at: w.tick,
        position: a.pos,
        course_deg: a.course_deg,
        altitude_m: a.altitude_m,
    })
}
fn prepare(w: &World, actor: EntityId, request: Request) -> Result<Prepared, String> {
    if !active(w) || request.delivery != Delivery::Strafe {
        return Err("9.6.9 explicit v4 Strafe delivery required".into());
    }
    let a = w
        .entity(actor)
        .filter(|e| e.alive && e.kind == PlatformKind::Aircraft)
        .ok_or("9.6.9 own live aircraft required")?;
    let profile = &tables()["aircraft"][&a.class];
    if profile.is_null() {
        return Err("9.6.9 fitted integral gun/caliber source unavailable".into());
    }
    let c = w.sides[a.side.index()]
        .contacts
        .get(&request.contact)
        .filter(|c| {
            !c.stale && !c.bearing_only && c.estimate.x.is_finite() && c.estimate.y.is_finite()
        })
        .ok_or("9.6.9 own current located contact required")?;
    let kind = super::detection2026::known_category(w, a.side, c.about)
        .filter(|k| matches!(k, PlatformKind::Ship | PlatformKind::Base))
        .ok_or("9.6.9 own known naval/ground category required")?;
    let (point, aim_source, native) = if request.aimpoint.is_empty() {
        let report=w.rules.aaw.as_ref().and_then(|s|s.firecontrol.tracks.values().filter(|t|t.observer==actor&&t.target==c.about).filter_map(|t|t.report.as_ref()).filter(|r|r.at<=w.tick&&w.tick-r.at<=1&&r.sensor_kind==crate::db::SensorKind::Visual&&r.measured_position.is_some_and(|point|point.x.is_finite()&&point.y.is_finite())&&a.sensors.iter().any(|m|m.sensor==r.sensor&&!crate::sensors::sensor_damaged(a,&m.sensor))).max_by_key(|r|r.at)).ok_or("9.6.9 actual onboard measured Visual report required; fused contact creates no free observation")?;
        let point = report.measured_position.unwrap();
        (
            point,
            "English2026 9.6.2 own measured Visual target".into(),
            Some(surface::NativeAim {
                observer: actor,
                sensor: report.sensor.clone(),
                at: report.at,
                measured_position: point,
            }),
        )
    } else {
        let aim = av::surface_aimpoint(w, a.side, &request.aimpoint)
            .ok_or("9.6.9 own authorized fixed aimpoint unavailable")?;
        if c.estimate.range_to(aim.point)
            > ac::tables()["surface_attack"]["tolerance_nm"]
                .as_f64()
                .unwrap()
        {
            return Err("9.6.9 own contact differs from allocated known point".into());
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
        known_kind: kind,
        point,
        aim_source,
        native,
        gun: profile["gun"].as_str().unwrap().into(),
        gun_rating: profile["gun_rating"].as_f64().unwrap(),
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
        &json!({"request":p.request,"gun":p.gun}),
    );
    Ok(p)
}
pub fn declare(
    w: &mut World,
    actor: EntityId,
    contact: ContactId,
    delivery: Delivery,
    aimpoint: &str,
) -> Result<(), String> {
    let p = prepare(
        w,
        actor,
        Request {
            contact,
            delivery,
            aimpoint: aimpoint.into(),
        },
    )?;
    let s = w.rules.strafe.as_mut().unwrap();
    if s.pending.contains_key(&actor) {
        return Err("9.6.9 a source gun pass is already pending".into());
    }
    let list = s.drafts.entry(actor).or_default();
    if !list.iter().any(|old| old.request == p.request) {
        list.push(p);
    }
    Ok(())
}
/// Existing SurfaceAttack JSON stays source-exact: rounds1 is one abstract
/// Salvo1 pass, not one shell, and its profile ID must match this airframe.
pub fn declare_legacy(
    w: &mut World,
    actor: EntityId,
    contact: ContactId,
    weapon: &str,
    rounds: u16,
    delivery: Delivery,
    aimpoint: &str,
) -> Result<(), String> {
    let a = w.entity(actor).ok_or("9.6.9 own aircraft missing")?;
    if rounds != 1 || tables()["aircraft"][&a.class]["gun"] != weapon {
        return Err("9.6.9 exact fitted source gun ID and one abstract pass required".into());
    }
    declare(w, actor, contact, delivery, aimpoint)
}
fn abort(w: &mut World, p: &Prepared, reason: String) {
    budget::abort(w, &p.identity);
    surface::finish_defense(w, p.actor);
    w.rules.strafe.as_mut().unwrap().pending.remove(&p.actor);
    w.rules
        .strafe
        .as_mut()
        .unwrap()
        .own_status
        .insert(p.actor, "cancelled".into());
    trace(
        w,
        p.actor,
        "9.6.9.pass_aborted",
        json!({"identity":p.identity,"reason":reason,"fired":false}),
    );
}
pub fn before_defense_scan(w: &mut World) {
    if !active(w) {
        return;
    }
    let drafts = std::mem::take(&mut w.rules.strafe.as_mut().unwrap().drafts);
    for (actor, mut variants) in drafts {
        if variants.len() != 1 {
            budget::cancel_actor(w, actor);
            trace(w, actor, "9.6.9.pass_conflict", json!({"fired":false}));
            continue;
        }
        let p = variants.pop().unwrap();
        if let Err(reason) = own_ready(w, &p).and_then(|_| surface::request_defense(w, actor)) {
            abort(w, &p, reason);
            continue;
        }
        w.rules.strafe.as_mut().unwrap().pending.insert(actor, p);
        w.rules
            .strafe
            .as_mut()
            .unwrap()
            .own_status
            .insert(actor, "pending".into());
    }
    let pending: Vec<_> = w
        .rules
        .strafe
        .as_ref()
        .unwrap()
        .pending
        .values()
        .cloned()
        .collect();
    for p in pending {
        if let Err(reason) = budget::submit(
            w,
            budget::Claim {
                identity: p.identity.clone(),
                actor: p.actor,
                target: p.target,
                known_kind: p.known_kind,
                kind: budget::AirAttackKind::UnguidedBomb,
            },
        ) {
            abort(w, &p, reason);
        }
    }
}
fn current_anchor(w: &World, p: &Prepared) -> Result<surface::ReleaseAnchor, String> {
    let straight = w.entity(p.actor).is_some_and(|a| {
        crate::geom::angle_diff(a.course_deg, p.anchor.course_deg).abs() <= 1e-9
            && crate::geom::angle_diff(a.ordered_course_deg, p.anchor.course_deg).abs() <= 1e-9
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
        .strafe
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
                "9.6.9.source_pending",
                json!({"identity":p.identity,"reason":reason,"fired":false}),
            );
            continue;
        }
        w.rules
            .strafe
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
        .strafe
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
            abort(w, &p, "9.6.9 source defense epoch/ticket changed".into());
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
        let gun = match gun_definition(w, &p) {
            Ok(gun) => gun,
            Err(reason) => {
                abort(w, &p, reason);
                continue;
            }
        };
        // Commit a real source pass and emission before hidden target truth.
        // No shell count/mass or other mounted store is fabricated/consumed.
        *w.rules
            .strafe
            .as_mut()
            .unwrap()
            .fired_passes
            .entry(p.actor)
            .or_default() += 1;
        w.emit(EventKind::RulesEvent{domain:"strafe2026".into(),rule:"9.6.9.guns_fired".into(),side:Some(p.side),unit:Some(p.actor),contact:Some(p.request.contact),detail:json!({"gun":p.gun,"gun_rating":p.gun_rating,"caliber_mm":gun.rule_data.as_ref().unwrap().diameter_mm,"attack_passes":1,"physical_round_count":Value::Null,"ammunition_model":tables()["ammunition_model"]}).to_string()},Visibility::Side(p.side));
        match impact::resolve(w, &p, &gun, no_aa, heavy_aa) {
            Ok(result) => {
                let hits = u8::from(result["hit"].as_bool().unwrap_or(false));
                let name = w.entity(p.actor).unwrap().name.clone();
                w.emit(
                    EventKind::GunAction {
                        side: p.side,
                        shooter: p.actor,
                        shooter_name: name,
                        target_contact: p.request.contact,
                        hits,
                    },
                    Visibility::Side(p.side),
                );
                w.rules
                    .strafe
                    .as_mut()
                    .unwrap()
                    .outcomes
                    .insert(p.identity.clone(), result.clone());
                trace(w, p.actor, "9.6.9.source_strafe_attack", result);
            }
            Err(reason) => {
                let result = json!({"identity":p.identity,"reason":reason,"source_pass_already_fired":true,"physical_round_count":Value::Null,"ammunition_model":tables()["ammunition_model"]});
                w.rules
                    .strafe
                    .as_mut()
                    .unwrap()
                    .outcomes
                    .insert(p.identity.clone(), result.clone());
                trace(w, p.actor, "9.6.9.source_pending", result);
            }
        }
        w.rules.strafe.as_mut().unwrap().pending.remove(&p.actor);
        w.rules
            .strafe
            .as_mut()
            .unwrap()
            .own_status
            .insert(p.actor, "fired".into());
        surface::finish_defense(w, p.actor);
        budget::finish(w, &p.identity);
    }
}
pub fn own_view(w: &World, actor: EntityId) -> Option<Value> {
    if !active(w) {
        return None;
    }
    let a = w.entity(actor)?;
    let s = w.rules.strafe.as_ref()?;
    if !s.config.enabled_aircraft.contains(&a.class) {
        return None;
    }
    let profile = &tables()["aircraft"][&a.class];
    let gun = profile["gun"].as_str()?;
    Some(
        json!({"gun":gun,"gun_rating":profile["gun_rating"],"caliber_mm":tables()["guns"][gun]["caliber_mm"],"ammunition_model":tables()["ammunition_model"],"physical_round_count":Value::Null,"fired_passes":s.fired_passes.get(&actor).copied().unwrap_or(0),"status":s.own_status.get(&actor),"pending":s.pending.get(&actor).map(|p|json!({"contact":p.request.contact,"delivery":p.request.delivery,"aimpoint":p.request.aimpoint,"accepted_at":p.accepted_at}))}),
    )
}

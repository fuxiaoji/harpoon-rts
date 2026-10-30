//! §8.5.4.4, distinct physical aircraft depth-charge patterns. Release
//! geometry and unverified hardware are explicit authored policy; §9.7.1
//! torpedo speed/altitude/rate restrictions are not silently inherited.
use super::depth_charge::Action;
use super::*;
use crate::types::PlatformKind;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit {
    pub source: String,
    pub payload: Payload,
    pub damage_major: f64,
    pub damage_minor: f64,
    pub max_depth_band: usize,
    pub units: BTreeMap<String, UnitFit>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Payload {
    Conventional,
    Homing { generation: u8 },
    ClusterRbk100,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitFit {
    pub source: String,
    pub mount_index: usize,
    pub round_mass_kg: f64,
    pub release_policy: ReleasePolicy,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleasePolicy {
    pub source: String,
    pub maximum_datum_range_nm: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unit {
    name: String,
    class: String,
    owner: Side,
    fit: UnitFit,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    units: BTreeMap<EntityId, BTreeMap<String, Unit>>,
    requests: Vec<(EntityId, Action)>,
    patterns: BTreeMap<String, Pattern>,
    next: u64,
    incoming: BTreeMap<EntityId, u64>,
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
struct Physical {
    alive: bool,
    submerged: bool,
    depth_m: f64,
    depth_band: usize,
    speed_kts: f64,
    cannot_evade: bool,
    acoustic_countermeasure: bool,
    target_class: String,
    aim_error_nm: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Pattern {
    id: String,
    alias: String,
    actor: EntityId,
    owner: Side,
    target: EntityId,
    contact: ContactId,
    weapon: String,
    charges: u16,
    tick: i64,
    aim: Vec2,
    fit: Fit,
    physical: Physical,
    accurate_depth: bool,
    depth_evidence: Value,
    declared_depth: Option<usize>,
    depth_report: Option<[usize; 3]>,
    depth_selected: bool,
    evading: Option<bool>,
    result: Option<Value>,
}
fn f(key: &str) -> f64 {
    tables()["air_depth_charge"][key]
        .as_f64()
        .expect("verified air DC source constant")
}
fn state(w: &World) -> Result<&State, String> {
    w.rules
        .asw_weapons
        .as_ref()
        .map(|s| &s.air_depth_charges)
        .ok_or("8.5.4.4 source state absent".into())
}
fn fit<'a>(w: &'a World, weapon: &str) -> Result<&'a Fit, String> {
    w.rules
        .asw_weapons
        .as_ref()
        .and_then(|s| s.config.air_depth_charges.get(weapon))
        .ok_or("8.5.4.4 exact sourced air DC fit absent".into())
}
pub(super) fn is_action(a: &Action) -> bool {
    matches!(
        a,
        Action::AirDepthChargePattern { .. }
            | Action::AirDepthChargeDepthReport { .. }
            | Action::AirDepthChargeSelectDepth { .. }
    )
}
pub(super) fn configured(w: &World, actor: EntityId, weapon: &str) -> bool {
    active(w) && state(w).is_ok_and(|s| s.units.get(&actor).is_some_and(|m| m.contains_key(weapon)))
}
fn named(w: &World, name: &str) -> Result<EntityId, String> {
    let mut ids = w.entities.values().filter(|e| e.name == name);
    let id = ids
        .next()
        .ok_or("8.5.4.4 exact original aircraft absent")?
        .id;
    if ids.next().is_some() {
        return Err("8.5.4.4 original aircraft name ambiguous".into());
    }
    Ok(id)
}
fn fit_numbers(v: &Fit) -> Result<(), String> {
    if v.source.trim().is_empty()
        || v.units.is_empty()
        || !v.damage_major.is_finite()
        || !v.damage_minor.is_finite()
        || v.damage_minor <= 0.
        || v.damage_major < v.damage_minor
        || !(1..=7).contains(&v.max_depth_band)
        || matches!(v.payload,Payload::Homing{generation}if !(1..=2).contains(&generation))
    {
        return Err(
            "8.5.4.4 independent sourced payload/guidance/depth/positive DP required".into(),
        );
    }
    Ok(())
}
pub(super) fn initialize(w: &World, c: &super::Configuration) -> Result<State, String> {
    let mut s = State::default();
    for (weapon, v) in &c.air_depth_charges {
        fit_numbers(v)?;
        if c.depth_charges.contains_key(weapon)
            || c.projectors.contains_key(weapon)
            || c.standoffs.contains_key(weapon)
            || !w.db.weapons.get(weapon).is_some_and(|d| {
                d.targets.sub && matches!(d.kind, WeaponKind::Bomb | WeaponKind::AswRocket)
            })
        {
            return Err("8.5.4.4 air DC store must bind its own actual weapon, separate from ship/projector/standoff execution".into());
        }
        for (name, u) in &v.units {
            let id = named(w, name)?;
            let e = w.entity(id).unwrap();
            if e.kind != PlatformKind::Aircraft
                || u.source.trim().is_empty()
                || u.release_policy.source.trim().is_empty()
                || !u.round_mass_kg.is_finite()
                || u.round_mass_kg < 0.
                || !u.release_policy.maximum_datum_range_nm.is_finite()
                || u.release_policy.maximum_datum_range_nm <= 0.
                || !e
                    .weapons
                    .get(u.mount_index)
                    .is_some_and(|m| m.weapon == *weapon && m.mounts > 0)
                || u.round_mass_kg == 0.
                    && !(u.source.starts_with("scenario_assumption")
                        && v.source.starts_with("scenario_assumption"))
            {
                return Err("8.5.4.4 actual original aircraft/store, explicit mass and authored release mechanics required; zero mass is experimental only".into());
            }
            s.units.entry(id).or_default().insert(
                weapon.clone(),
                Unit {
                    name: name.clone(),
                    class: e.class.clone(),
                    owner: e.side,
                    fit: u.clone(),
                },
            );
        }
    }
    Ok(s)
}
fn actor<'a>(
    w: &'a World,
    s: &'a State,
    id: EntityId,
    weapon: &str,
    charges: u16,
) -> Result<(&'a Unit, &'a Fit), String> {
    let u = s
        .units
        .get(&id)
        .and_then(|m| m.get(weapon))
        .ok_or("8.5.4.4 no exact own air DC equipment")?;
    let e = w
        .entity(id)
        .filter(|e| {
            e.alive
                && e.kind == PlatformKind::Aircraft
                && e.name == u.name
                && e.class == u.class
                && e.side == u.owner
        })
        .ok_or("8.5.4.4 original aircraft identity/availability changed")?;
    super::super::aircombat2026::surface_attack_gate(w, id)?;
    if !super::super::unmanned2026::can_attack(w, id)
        || !super::super::expanded_damage2026::platform_weapons_operational(w, id)
        || !super::super::expanded_damage2026::weapon_operational(w, id, weapon)?
    {
        return Err("8.5.4.4 actual aircraft/control/store does not permit release".into());
    }
    super::super::nuclear2026::release_gate(w, id, weapon)?;
    let fit = fit(w, weapon)?;
    fit_numbers(fit)?;
    let maximum = f(if matches!(fit.payload, Payload::ClusterRbk100) {
        "rbk_max_bombs"
    } else {
        "maximum_charges"
    }) as u16;
    if charges == 0 || charges > maximum {
        return Err("8.5.4.4 source permits at most eight stores in one aircraft attack".into());
    }
    let m = e
        .weapons
        .get(u.fit.mount_index)
        .filter(|m| m.weapon == weapon && m.mounts > 0 && m.ammo >= charges)
        .ok_or("8.5.4.4 original physical store stock unavailable")?;
    let live = super::super::expanded_damage2026::physical_weapon_mounts(w, id, u.fit.mount_index)?;
    if live.len() != 1 || m.mounts != 1 {
        return Err("8.5.4.4 one exact surviving physical store rack required; multiple-rack allocation not inferred".into());
    }
    if super::super::expanded_damage2026::physical_weapon_stock(
        w,
        id,
        weapon,
        u.fit.mount_index,
        live[0],
    )?
    .is_some_and(|n| n != m.ammo)
    {
        return Err("8.5.4.4 physical rack and aggregate store inventory disagree".into());
    }
    let flight = w
        .rules
        .aviation
        .as_ref()
        .and_then(|s| s.flights.get(&id))
        .filter(|f| f.fuel_nm.is_finite() && f.fuel_nm > 0. && f.payload_fraction.is_finite())
        .ok_or("8.5.4.4 actual source Flight/load/fuel state required")?;
    if u.fit.round_mass_kg > 0.
        && (flight
            .external_weapon_rounds
            .get(weapon)
            .copied()
            .unwrap_or(0)
            < charges
            || !flight.spec.max_payload_kg.is_finite()
            || flight.spec.max_payload_kg <= 0.
            || flight.payload_fraction * flight.spec.max_payload_kg + 1e-9
                < f64::from(charges) * u.fit.round_mass_kg)
    {
        return Err("8.5.4.4 exact physical aircraft store mass/inventory unavailable".into());
    }
    Ok((u, fit))
}
fn preflight(
    w: &World,
    s: &State,
    id: EntityId,
    contact: ContactId,
    weapon: &str,
    charges: u16,
    depth: Option<usize>,
) -> Result<(EntityId, Vec2), String> {
    let (u, v) = actor(w, s, id, weapon, charges)?;
    let e = w.entity(id).unwrap();
    if matches!(v.payload, Payload::Homing { .. }) {
        if depth.is_some() {
            return Err("8.5.4.4 homing searches its real depth envelope; a guessed fuze band is inapplicable".into());
        }
    } else if depth.is_none_or(|b| b == 0 || b > v.max_depth_band) {
        return Err(
            "8.5.4.4 explicit conventional/cluster depth within source envelope required".into(),
        );
    }
    let c = w.sides[e.side.index()]
        .contacts
        .get(&contact)
        .filter(|c| !c.stale && !c.bearing_only)
        .ok_or("8.5.4.4 own current located ContactId required")?;
    let aim=super::depth_charge::located_aim(w,id,c.about)?.ok_or("8.5.4.4 own native or genuinely delivered location required; fused CID alone is not communication")?;
    if !aim.x.is_finite()
        || !aim.y.is_finite()
        || !e.pos.x.is_finite()
        || !e.pos.y.is_finite()
        || e.pos.range_to(aim) > u.fit.release_policy.maximum_datum_range_nm
    {
        return Err("8.5.4.4 aircraft outside explicit measured-datum release geometry".into());
    }
    Ok((c.about, aim))
}
pub(super) fn validate_action(w: &World, id: EntityId, a: &Action) -> Result<(), String> {
    let e = w
        .entity(id)
        .filter(|e| e.alive)
        .ok_or("8.5.4.4 own live actor absent")?;
    if !active(w) || w.over || crate::exercise::locked(w, e.side) {
        return Err("8.5.4.4 active Full-profile exercise required".into());
    }
    let s = state(w)?;
    match a {
        Action::AirDepthChargePattern {
            contact,
            weapon,
            charges,
            depth_band,
        } => {
            preflight(w, s, id, *contact, weapon, *charges, *depth_band)?;
        }
        Action::AirDepthChargeDepthReport { attack, bands } => {
            let p = s
                .patterns
                .values()
                .find(|p| {
                    p.alias == *attack
                        && p.target == id
                        && p.result.is_none()
                        && !p.accurate_depth
                        && p.depth_report.is_none()
                })
                .ok_or("8.5.4.4 own unresolved incoming depth report required")?;
            let distinct: BTreeSet<_> = bands.iter().copied().collect();
            if distinct.len() != 3
                || bands.iter().any(|b| *b == 0 || *b > 7)
                || !bands.contains(&p.physical.depth_band)
            {
                return Err("8.5.4.4 three distinct possible bands must include the original actual depth exactly once".into());
            }
        }
        Action::AirDepthChargeSelectDepth { attack, depth_band } => {
            let p=s.patterns.get(attack).filter(|p|p.actor==id&&p.result.is_none()&&p.depth_report.is_some()&&!p.depth_selected).ok_or("8.5.4.4 original aircraft must receive the depth choices before one final selection")?;
            if *depth_band == 0 || *depth_band > p.fit.max_depth_band {
                return Err("8.5.4.4 final selected depth outside physical weapon envelope".into());
            }
        }
        _ => return Err("8.5.4.4 wrong action domain".into()),
    }
    Ok(())
}
pub(super) fn control(w: &mut World, id: EntityId, mut a: Action) -> Result<(), String> {
    if let Action::AirDepthChargeDepthReport { bands, .. } = &mut a {
        bands.sort_unstable();
    }
    validate_action(w, id, &a)?;
    w.rules
        .asw_weapons
        .as_mut()
        .unwrap()
        .air_depth_charges
        .requests
        .push((id, a));
    Ok(())
}
fn depth_knowledge(w: &World, actor: EntityId, target: EntityId) -> (bool, Value) {
    let Some(e) = w.entity(actor) else {
        return (false, Value::Null);
    };
    let freshness = super::super::firecontrol2026::tables()["execution_revision"]["own_motion"]
        ["report_freshness_s"]
        .as_i64()
        .unwrap();
    let dipping_geometry = e.speed_kts.is_finite()
        && e.speed_kts
            <= super::super::detection2026::tables()["sonar"]["dipping"]
                ["max_horizontal_speed_kts"]
                .as_f64()
                .unwrap()
        && w.rules
            .detection
            .as_ref()
            .and_then(|s| s.config.dipping_platforms.get(&e.class))
            .is_some_and(|p| match p {
                super::super::detection2026::DippingPlatform::Helicopter => e.altitude_m > 0.,
                super::super::detection2026::DippingPlatform::FlyingBoat => e.altitude_m == 0.,
            });
    let current = w
        .rules
        .aaw
        .as_ref()
        .into_iter()
        .flat_map(|s| s.firecontrol.tracks.values())
        .filter(|t| t.observer == actor && t.target == target)
        .filter_map(|t| t.report.as_ref())
        .find(|r| {
            dipping_geometry
                && r.at <= w.tick
                && w.tick - r.at <= freshness
                && r.measured_position.is_some()
                && r.sensor_kind == SensorKind::SonarActive
                && matches!(
                    &r.receiver,
                    Some(super::super::detection2026::Receiver::Sonar {
                        generation: 4..=7,
                        mount: super::super::detection2026::SonarMount::Dipping,
                        ..
                    })
                )
                && e.sensors.iter().any(|s| {
                    s.sensor == r.sensor
                        && s.active
                        && !crate::sensors::sensor_damaged(e, &s.sensor)
                })
                && super::super::expanded_damage2026::sensor_operational(w, actor, &r.sensor)
                    .unwrap_or(false)
                && w.rules
                    .detection
                    .as_ref()
                    .and_then(|s| s.arrays.get(&format!("{}:{}", actor.0, r.sensor)))
                    .is_some_and(|j| j.deployed && !j.lost && !j.recovering && j.ready_at <= w.tick)
        });
    if let Some(r) = current {
        return (
            true,
            json!({"source":"8.5.4.4 actual current deployed Gen4+ active dipping report","observer":actor,"sensor":r.sensor,"at":r.at}),
        );
    }
    // A real visual/IR/radar submarine observation supplies the source's
    // Shallow datum. Neither a generic sonar report nor a fused icon does.
    let native_surface = w
        .rules
        .aaw
        .as_ref()
        .into_iter()
        .flat_map(|s| s.firecontrol.tracks.values())
        .filter(|t| t.observer == actor && t.target == target)
        .filter_map(|t| t.report.as_ref())
        .find(|r| {
            r.at <= w.tick
                && w.tick - r.at <= freshness
                && r.measured_position.is_some()
                && matches!(
                    r.sensor_kind,
                    SensorKind::Visual | SensorKind::Ir | SensorKind::Radar
                )
                && e.sensors
                    .iter()
                    .any(|s| s.sensor == r.sensor && !crate::sensors::sensor_damaged(e, &s.sensor))
        })
        .cloned();
    let surface = native_surface.or_else(|| {
        super::super::communications2026::received_located_report(w, actor, target)
            .ok()
            .flatten()
            .filter(|r| {
                matches!(
                    r.sensor_kind,
                    SensorKind::Visual | SensorKind::Ir | SensorKind::Radar
                )
            })
    });
    if let Some(r) = surface {
        return (
            true,
            json!({"source":"8.5.4.4 actual visual/IR/radar contact gives Shallow datum","observer":r.observer,"sensor":r.sensor,"at":r.at,"known_depth_band":1}),
        );
    }
    (
        false,
        json!({"source":"8.5.4.4 three possible depth bands; no current own active Gen4+ dipping or visual/IR/radar datum"}),
    )
}
fn physical(w: &World, target: EntityId, aim: Vec2) -> Physical {
    let Some(e) = w.entity(target) else {
        return Physical {
            alive: false,
            submerged: false,
            depth_m: 0.,
            depth_band: usize::MAX,
            speed_kts: 0.,
            cannot_evade: false,
            acoustic_countermeasure: false,
            target_class: String::new(),
            aim_error_nm: 0.,
        };
    };
    let maximum = e.max_speed_with_depth_rules(
        &w.db,
        super::super::navigation2026::submarine::configured_depth(w, target),
    );
    let depth = super::super::detection2026::depth_band(e.depth_m).unwrap_or(usize::MAX);
    Physical {
        alive: e.alive,
        submerged: e.kind == PlatformKind::Sub && e.depth_m > 0.,
        depth_m: e.depth_m,
        depth_band: if e.depth_m > 0. && depth == 0 {
            1
        } else {
            depth
        },
        speed_kts: e.speed_kts.abs(),
        cannot_evade: maximum.is_finite() && maximum <= 0.
            || super::super::expanded_damage2026::engineering_available(w, target)
                .is_ok_and(|v| !v),
        acoustic_countermeasure: !super::super::ew2026::active_acms(w, target).is_empty(),
        target_class: e.class.clone(),
        aim_error_nm: aim.range_to(e.pos),
    }
}
fn commit(w: &mut World, s: &mut State, id: EntityId, a: &Action) -> Result<(), String> {
    let Action::AirDepthChargePattern {
        contact,
        weapon,
        charges,
        depth_band,
    } = a
    else {
        unreachable!()
    };
    let (target, aim) = preflight(w, s, id, *contact, weapon, *charges, *depth_band)?;
    let u = s.units[&id][weapon].clone();
    let v = fit(w, weapon)?.clone();
    let sequence = s
        .next
        .checked_add(1)
        .ok_or("8.5.4.4 pattern identity exhausted")?;
    let incoming = s
        .incoming
        .get(&target)
        .copied()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("8.5.4.4 private incoming identity exhausted")?;
    // Source-provenance and own stock gates finish before hidden target facts.
    let physical_rack =
        super::super::expanded_damage2026::physical_weapon_mounts(w, id, u.fit.mount_index)?[0];
    super::super::expanded_damage2026::spend_physical_weapon_stock(
        w,
        id,
        weapon,
        u.fit.mount_index,
        physical_rack,
        *charges,
    )?;
    w.entity_mut(id).unwrap().weapons[u.fit.mount_index].ammo -= *charges;
    if u.fit.round_mass_kg > 0. {
        let flight = w
            .rules
            .aviation
            .as_mut()
            .unwrap()
            .flights
            .get_mut(&id)
            .unwrap();
        *flight.external_weapon_rounds.get_mut(weapon).unwrap() -= *charges;
        flight.payload_fraction = (flight.payload_fraction
            - f64::from(*charges) * u.fit.round_mass_kg / flight.spec.max_payload_kg)
            .max(0.);
    }
    let (accurate_depth, evidence) = depth_knowledge(w, id, target);
    let physical = physical(w, target, aim);
    let evading = w
        .rules
        .asw_weapons
        .as_ref()
        .and_then(|s| s.depth_charges.defense.get(&target))
        .map(|v| v.evading);
    let homing = matches!(v.payload, Payload::Homing { .. });
    let identity = format!("8.5.4.4:{}:{sequence}", id.0);
    s.next = sequence;
    s.incoming.insert(target, incoming);
    s.patterns.insert(
        identity.clone(),
        Pattern {
            id: identity.clone(),
            alias: format!("own-adc-{}-{incoming}", target.0),
            actor: id,
            owner: u.owner,
            target,
            contact: *contact,
            weapon: weapon.clone(),
            charges: *charges,
            tick: w.tick,
            aim,
            fit: v,
            physical,
            accurate_depth: homing || accurate_depth,
            depth_evidence: evidence,
            declared_depth: *depth_band,
            depth_report: None,
            depth_selected: homing || accurate_depth,
            evading,
            result: None,
        },
    );
    w.emit(
        EventKind::WeaponsRelease {
            side: u.owner,
            shooter: id,
            shooter_name: u.name.clone(),
            weapon: w.db.weapons[weapon].name.clone(),
            count: *charges as u8,
        },
        Visibility::Side(u.owner),
    );
    trace(
        w,
        id,
        "8.5.4.4.pattern_committed",
        json!({"attack":identity,"weapon":weapon,"charges":charges,"physical_stock_debited":true,"mechanics_source":u.fit.release_policy.source}),
    );
    Ok(())
}
fn adjudicate(w: &World, p: &Pattern) -> Option<Value> {
    let x = &p.physical;
    let mut r = json!({"attack":p.id,"weapon":p.weapon,"charges":p.charges,"payload":p.fit.payload,"distribution":"dispersed","actual_depth_m":x.depth_m,"actual_depth_band":x.depth_band,"actual_speed_kts":x.speed_kts,"aim_error_nm":x.aim_error_nm,"base_percent":null,"homing_multiplier":1.,"countermeasure_multiplier":1.,"major_threshold":null,"minor_threshold":null,"raw_d100":null,"modified_d100":null,"severity":"miss","dp":null,"damage_queued":false,"reason":null});
    if !x.alive || !x.submerged {
        r["reason"] = json!("no live submerged submarine at actual release");
        return Some(r);
    }
    if x.depth_band > p.fit.max_depth_band {
        r["reason"] = json!("real target below the store maximum search/fuze depth");
        return Some(r);
    }
    if !p.depth_selected {
        return None;
    }
    if !matches!(p.fit.payload, Payload::Homing { .. }) && p.declared_depth != Some(x.depth_band) {
        r["reason"] = json!("final declared band missed the original actual depth");
        return Some(r);
    }
    if x.depth_m
        > tables()["depth_charge"]["matrix_deep_i_max_m"]
            .as_f64()
            .unwrap()
    {
        r["severity"] = json!("pending");
        r["reason"] = json!("source speed/depth matrix ends at DeepI; no DeepII cell inferred");
        return Some(r);
    }
    // Captured measured aim is not silently moved to a later physical target.
    // The source has no arbitrary drift-radius conversion for this aircraft
    // subset, so unmatched placement remains explicit referee source pending.
    if x.aim_error_nm > 1e-9 {
        r["severity"] = json!("pending");
        r["reason"]=json!("captured aircraft datum differs from actual release target; source placement/drift conversion unresolved");
        return Some(r);
    }
    let evading = if x.cannot_evade { false } else { p.evading? };
    let factor = match super::depth_charge::modifier(x.speed_kts, x.depth_band) {
        Ok(v) => v,
        Err(reason) => {
            r["severity"] = json!(if reason.contains("not possible") {
                "miss"
            } else {
                "pending"
            });
            r["reason"] = json!(reason);
            return Some(r);
        }
    };
    let multiplier = match p.fit.payload {
        Payload::Conventional => 1.,
        Payload::Homing { generation: 1 } => f("homing_gen1_multiplier"),
        Payload::Homing { generation: 2 } => f("homing_gen2_multiplier"),
        Payload::Homing { .. } => unreachable!(),
        Payload::ClusterRbk100 => f("rbk_percent_per_bomb"),
    };
    let cm = if matches!(p.fit.payload, Payload::Homing { .. }) && x.acoustic_countermeasure {
        f("acoustic_countermeasure_factor")
    } else {
        1.
    };
    let base = f64::from(p.charges) * multiplier;
    let major = (base * factor * cm).round() as u32;
    let raw = super::die(w, p.actor, p.tick, &format!("{}:air_dc_d100", p.id), 100);
    let modified = i32::from(raw)
        + if evading {
            0
        } else {
            tables()["depth_charge"]["non_evading_d100_modifier"]
                .as_i64()
                .unwrap() as i32
        };
    r["base_percent"] = json!(base);
    r["speed_depth_multiplier"] = json!(factor);
    r["homing_multiplier"] = json!(if matches!(p.fit.payload, Payload::Homing { .. }) {
        multiplier
    } else {
        1.
    });
    r["countermeasure_multiplier"] = json!(cm);
    r["major_threshold"] = json!(major);
    r["minor_threshold"] = json!(major.saturating_mul(2));
    r["raw_d100"] = json!(raw);
    r["modified_d100"] = json!(modified);
    if matches!(p.fit.payload, Payload::ClusterRbk100) {
        let hits = u8::from(modified <= major as i32);
        r["cluster_max_submunition_hits"] = json!(1);
        r["submunition_hits"] = json!(hits);
        r["minor_threshold"] = Value::Null;
        r["severity"] = json!(if hits == 1 { "contact" } else { "miss" });
        if hits == 1 {
            r["reason"]=json!("RBK contact requires its independent typed ASW pressure-hull check, then actual Minor column on failed penetration");
        }
    } else if modified <= major as i32 {
        r["severity"] = json!("major");
        r["dp"] = json!(p.fit.damage_major)
    } else if modified <= major.saturating_mul(2) as i32 {
        r["severity"] = json!("minor");
        r["dp"] = json!(p.fit.damage_minor)
    }
    Some(r)
}
pub(super) fn resolve(w: &mut World) {
    if !active(w) {
        return;
    }
    let Some(mut s) = w
        .rules
        .asw_weapons
        .as_mut()
        .map(|s| std::mem::take(&mut s.air_depth_charges))
    else {
        return;
    };
    let mut requests: BTreeMap<(u8, EntityId, String), BTreeMap<String, Action>> = BTreeMap::new();
    for (actor, a) in std::mem::take(&mut s.requests) {
        let (phase, key) = match &a {
            Action::AirDepthChargeDepthReport { attack, .. } => (0, attack.clone()),
            Action::AirDepthChargeSelectDepth { attack, .. } => (1, attack.clone()),
            Action::AirDepthChargePattern { .. } => (2, String::new()),
            _ => continue,
        };
        requests
            .entry((phase, actor, key))
            .or_default()
            .insert(serde_json::to_string(&a).unwrap(), a);
    }
    for ((_, actor, _), choices) in requests {
        if choices.len() != 1 {
            trace(
                w,
                actor,
                "8.5.4.4.request_rejected",
                json!({"reason":"different same-tick air pattern requests rejected as one conflicting set"}),
            );
            continue;
        }
        let a = choices.values().next().unwrap();
        // Revalidate against the local detached state without publishing an
        // incomplete snapshot or losing the original accepted requests.
        let error = match a {
            Action::AirDepthChargePattern {
                contact,
                weapon,
                charges,
                depth_band,
            } => preflight(w, &s, actor, *contact, weapon, *charges, *depth_band).map(|_| ()),
            Action::AirDepthChargeDepthReport { attack, bands } => s
                .patterns
                .values()
                .find(|p| {
                    p.alias == *attack
                        && p.target == actor
                        && p.result.is_none()
                        && !p.accurate_depth
                        && p.depth_report.is_none()
                })
                .filter(|p| {
                    bands.iter().copied().collect::<BTreeSet<_>>().len() == 3
                        && bands.iter().all(|b| *b > 0 && *b <= 7)
                        && bands.contains(&p.physical.depth_band)
                })
                .map(|_| ())
                .ok_or("8.5.4.4 original depth-report facts no longer eligible".into()),
            Action::AirDepthChargeSelectDepth { attack, depth_band } => s
                .patterns
                .get(attack)
                .filter(|p| {
                    p.actor == actor
                        && p.result.is_none()
                        && p.depth_report.is_some()
                        && !p.depth_selected
                        && *depth_band > 0
                        && *depth_band <= p.fit.max_depth_band
                })
                .map(|_| ())
                .ok_or("8.5.4.4 original depth-selection facts no longer eligible".into()),
            _ => unreachable!(),
        };
        if let Err(reason) = error {
            trace(
                w,
                actor,
                "8.5.4.4.request_rejected",
                json!({"reason":reason}),
            );
            continue;
        }
        match a {
            Action::AirDepthChargePattern { .. } => {
                if let Err(reason) = commit(w, &mut s, actor, a) {
                    trace(
                        w,
                        actor,
                        "8.5.4.4.request_rejected",
                        json!({"reason":reason}),
                    );
                }
            }
            Action::AirDepthChargeDepthReport { attack, bands } => {
                s.patterns
                    .values_mut()
                    .find(|p| p.alias == *attack && p.target == actor)
                    .unwrap()
                    .depth_report = Some(*bands);
            }
            Action::AirDepthChargeSelectDepth { attack, depth_band } => {
                let p = s.patterns.get_mut(attack).unwrap();
                p.declared_depth = Some(*depth_band);
                p.depth_selected = true;
            }
            _ => unreachable!(),
        }
    }
    for p in s.patterns.values_mut().filter(|p| p.result.is_none()) {
        if p.evading.is_none() {
            p.evading = w
                .rules
                .asw_weapons
                .as_ref()
                .and_then(|s| s.depth_charges.defense.get(&p.target))
                .map(|v| v.evading)
        }
        let Some(mut result) = adjudicate(w, p) else {
            continue;
        };
        if let Some(dp) = result["dp"].as_f64() {
            match super::super::damage_runtime::queue_depth_charge(
                w,
                p.actor,
                p.target,
                &p.weapon,
                dp,
                result["severity"] == "major",
                &p.id,
            ) {
                Ok(()) => result["damage_queued"] = json!(true),
                Err(reason) => result["reason"] = json!(reason),
            }
        }
        // Source cluster contact is a separate trusted common-layer cause:
        // penetration queues a pressure-hull effect with zero ordinary DP;
        // failed penetration queues the store's actual Minor DP/column.
        if result["severity"] == "contact" {
            match super::super::damage_runtime::queue_asw_cluster_contact(
                w,
                p.actor,
                p.target,
                &p.weapon,
                p.fit.damage_minor,
                super::super::damage_runtime::AswContactScale::Full,
                &p.id,
            ) {
                Ok(outcome) => {
                    result["damage_queued"] = json!(outcome.damage_queued);
                    result["dp"] = if outcome.minor_damage_queued {
                        json!(p.fit.damage_minor)
                    } else if outcome.hull_effect_queued {
                        json!(0.)
                    } else {
                        Value::Null
                    };
                    result["severity"] = json!(if outcome.pending_reason.is_some() {
                        "pending_contact_hull"
                    } else if outcome.penetrated == Some(true) {
                        "contact_pressure_hull"
                    } else {
                        "contact_minor"
                    });
                    if let Some(reason) = &outcome.pending_reason {
                        result["reason"] = json!(reason);
                    } else {
                        result["reason"] = Value::Null;
                    }
                    result["contact_hull"] = serde_json::to_value(outcome)
                        .expect("typed source contact outcome serializable");
                }
                Err(reason) => {
                    result["severity"] = json!("pending_contact_hull");
                    result["reason"] = json!(reason);
                }
            }
        }
        trace(w, p.actor, "8.5.4.4.pattern_result", result.clone());
        p.result = Some(result);
    }
    w.rules.asw_weapons.as_mut().unwrap().air_depth_charges = s;
}
pub(super) fn own_view(w: &World, id: EntityId) -> Value {
    let Ok(s) = state(w) else { return Value::Null };
    let Some(e) = w.entity(id) else {
        return Value::Null;
    };
    let weapons:BTreeMap<_,_>=s.units.get(&id).into_iter().flat_map(|m|m.iter()).filter(|(_,u)|u.owner==e.side).map(|(weapon,u)|(weapon.clone(),json!({"mount_index":u.fit.mount_index,"stock":e.weapons.get(u.fit.mount_index).map(|m|m.ammo),"source":u.fit.source}))).collect();
    let pending:Vec<_>=s.patterns.values().filter(|p|p.actor==id&&p.owner==e.side&&p.result.is_none()).map(|p|json!({"attack":p.id,"contact":p.contact,"weapon":p.weapon,"charges":p.charges,"depth_options_required":if p.accurate_depth{0}else{3},"depth_report":p.depth_report,"select_depth_required":p.depth_report.is_some()&&!p.depth_selected,"waiting_for_evasion_declaration":p.evading.is_none(),"depth_evidence":p.depth_evidence})).collect();
    let incoming:Vec<_>=s.patterns.values().filter(|p|p.target==id&&p.result.is_none()).map(|p|json!({"attack":p.alias,"own_depth_band_at_release":p.physical.depth_band,"depth_report_required":!p.accurate_depth&&p.depth_report.is_none(),"depth_options_required":if p.accurate_depth{0}else{3}})).collect();
    if weapons.is_empty() && pending.is_empty() && incoming.is_empty() {
        return Value::Null;
    }
    json!({"air_weapons":weapons,"air_pending_attacks":pending,"air_incoming_patterns":incoming})
}

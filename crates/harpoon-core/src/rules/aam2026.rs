//! Harpoon V2026 §9.5 source AAM packet executor. Explicit v4 only.
//! Physical flight is an explicit RTS adaptation; packet, ammunition, and D100 remain source-exact.
use super::aaw2026::{Countermeasures, Outcome, Solution};
use crate::{
    db::{Guidance, WeaponDef, WeaponKind},
    geom::angle_diff,
    types::PlatformKind,
    EntityId, Rng, Side, World,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};
pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/aam-2026.json")).unwrap()
    })
}
fn n(v: &Value) -> Result<f64, String> {
    v.as_f64()
        .ok_or_else(|| "9.5 required source numeric field is absent".into())
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub packets: BTreeMap<EntityId, Packet>,
    pub outcomes: BTreeMap<String, Outcome>,
    pub clocks: BTreeMap<EntityId, i64>,
    pub evasion_queue: Vec<(EntityId, EntityId)>,
    pub diagnostics: BTreeSet<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Packet {
    pub identity: String,
    pub shooter: EntityId,
    pub target: EntityId,
    pub weapon: String,
    pub tick: i64,
    pub members: Vec<EntityId>,
    pub attack: Attack,
    pub guidance: String,
    pub hoj: bool,
    /// Once the controller has evaded, a non-TARH missile cannot regain
    /// guidance merely because the incoming threat disappears before arrival.
    #[serde(default)]
    pub controller_evaded: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attack {
    pub ata: f64,
    pub mr: f64,
    pub maneuvering: bool,
    pub speed: f64,
    pub signature: usize,
    pub radar_generation: usize,
    pub snap_penalty: bool,
    pub dogfight_unrated: bool,
    pub countermeasures: Countermeasures,
    pub hoj: bool,
}
/// §4.10.2 +9.5.2 outside dogfight. Aspect is target's nose-to-launcher angle.
pub fn range_nm(
    base: f64,
    speed: f64,
    altitude: f64,
    target_speed: f64,
    aspect: f64,
    nonmaneuvering: bool,
    dogfight: bool,
) -> Result<f64, String> {
    if [base, speed, altitude, target_speed, aspect]
        .iter()
        .any(|v| !v.is_finite())
        || base < 0.
        || speed < 0.
        || target_speed < 0.
        || altitude < 0.
        || !(0.0..=180.0).contains(&aspect)
    {
        return Err("9.5.2 invalid or unknown range input".into());
    }
    if dogfight {
        return Ok(base);
    }
    let d = &tables()["range"];
    let mut range = base;
    if speed < n(&d["slow_launch_below_kts"])? {
        range *= n(&d["slow_launch_factor"])?
    }
    if speed > n(&d["fast_launch_above_kts"])? && altitude > 7500. && altitude <= 30000. {
        range *= n(&d["fast_launch_factor"])?
    }
    if altitude <= 2000. {
        range *= n(&d["low_factor"])?
    }
    if aspect >= 180. - n(&d["tail_half_angle"])? {
        range *= n(&d["tail_factor"])?
    }
    let combined = speed + target_speed;
    if nonmaneuvering
        && aspect <= n(&d["closing_half_angle"])?
        && combined >= n(&d["closing_min_combined"])?
    {
        range *= if combined >= 2001. { 2. } else { 1.5 };
    }
    Ok(range)
}
pub fn solution(a: &Attack, count: u8) -> Result<Solution, String> {
    if a.signature > 4 || !(1..=6).contains(&a.radar_generation) {
        return Err("9.5 radar/signature not verified".into());
    }
    let radar_key = match a.radar_generation {
        1 | 2 => "2",
        3 => "3",
        4 => "4",
        5 => "5h",
        6 => "6a",
        _ => unreachable!(),
    };
    let mut modifiers = BTreeMap::new();
    modifiers.insert(
        "maneuver_rating".into(),
        if a.maneuvering { -a.mr } else { 0. },
    );
    modifiers.insert("speed".into(), super::aaw2026::speed_ata(a.speed)?);
    modifiers.insert(
        "signature".into(),
        n(&super::aaw2026::tables()["signature_ata"][a.signature])?,
    );
    modifiers.insert(
        "air_intercept_radar".into(),
        n(&super::aaw2026::tables()["combat_system"][radar_key][1])?,
    );
    modifiers.insert(
        "different_altitude_no_snap".into(),
        if a.snap_penalty {
            n(&tables()["snap_penalty"])?
        } else {
            0.
        },
    );
    modifiers.insert(
        "countermeasures".into(),
        if a.hoj {
            0.
        } else {
            super::aaw2026::countermeasure_modifier(&a.countermeasures, a.maneuvering)?
        },
    );
    let mut ata = a.ata;
    if a.dogfight_unrated {
        ata *= n(&tables()["dogfight_unrated_factor"])?
    }
    if a.hoj {
        ata *= n(&tables()["hoj_factor"])?
    }
    let value = ata + modifiers.values().sum::<f64>();
    Ok(Solution {
        value,
        probability: super::aaw2026::missile_probability(value, count)?,
        modifiers,
    })
}
pub fn weapon_profile(id: &str) -> Result<&'static Value, String> {
    tables()["weapons"]
        .get(id)
        .ok_or_else(|| "9.5 Annex H4 exact variant not verified".into())
}
pub fn aircraft_profile(class: &str) -> Result<&'static Value, String> {
    tables()["aircraft"]
        .get(class)
        .ok_or_else(|| "9.5 fitted aircraft countermeasures/radar targets not verified".into())
}
pub fn aircraft_countermeasures(
    w: &World,
    target: EntityId,
    seeker: usize,
) -> Result<Countermeasures, String> {
    let e = w.entity(target).ok_or("aircraft absent")?;
    let p = aircraft_profile(&e.class)?;
    let available =
        super::expanded_damage2026::aircraft_profile(w, target, 0., false)?.countermeasures;
    Ok(Countermeasures {
        seeker_generation: seeker,
        jammer_generation: if available
            && super::expanded_damage2026::binding_operational(
                w,
                target,
                super::expanded_damage2026::ComponentKind::Countermeasure,
                "jammer",
            )? {
            p["jammer"].as_u64().map(|n| n as usize)
        } else {
            None
        },
        decoy_generation: if available
            && super::expanded_damage2026::binding_operational(
                w,
                target,
                super::expanded_damage2026::ComponentKind::Countermeasure,
                "decoy",
            )? {
            p["decoy"].as_u64().map(|n| n as usize)
        } else {
            None
        },
    })
}
/// Referee-only source target data. Callers must validate their own contact before using it.
pub fn target_data(
    w: &World,
    target: EntityId,
    observer: EntityId,
    seeker: usize,
) -> Result<(f64, bool, usize, Countermeasures), String> {
    Ok((
        super::aircombat2026::target_maneuver(w, target)?,
        super::aircombat2026::maneuvering(w, target),
        super::aircombat2026::target_signature(w, target, observer)?,
        aircraft_countermeasures(w, target, seeker)?,
    ))
}
fn target_speed(w: &World, target: EntityId) -> Result<f64, String> {
    let e = w.entity(target).ok_or("9.4 target aircraft missing")?;
    if super::aircombat2026::in_dogfight(w, target) {
        super::aviation2026::entity_source_speed(
            w,
            target,
            e.altitude_m,
            super::aviation2026::Throttle::Cruise,
        )
    } else {
        Ok(e.speed_kts)
    }
}
/// Source 9.5.1 is an irreversible guidance failure, saved on every member
/// of the committed packet. It does not depend on terminal entity order.
pub fn controller_evaded(w: &mut World, shooter: EntityId) {
    let Some(aaw) = w.rules.aaw.as_mut() else {
        return;
    };
    let mut spoiled = BTreeSet::new();
    for packet in aaw.aam.packets.values_mut() {
        if packet.shooter == shooter && packet.guidance != "TARH" && !packet.controller_evaded {
            packet.controller_evaded = true;
            spoiled.insert(packet.identity.clone());
        }
    }
    if !spoiled.is_empty() {
        super::aaw2026::trace(
            w,
            shooter,
            "9.5.1.guidance_lost",
            json!({"packet_count":spoiled.len(),"permanent":true}),
        );
    }
}
fn radar(w: &World, actor: EntityId, required: bool) -> Result<usize, String> {
    let e = w.entity(actor).ok_or("aircraft absent")?;
    if required
        && (!crate::sensors::radar_emitting(e)
            || e.systems.radar <= 0.05
            || !super::expanded_damage2026::aircraft_profile(w, actor, 0., false)?.sensors)
    {
        return Err("6.3/9.5 source radar-guided AAM requires operating radar".into());
    }
    e.sensors
        .iter()
        .filter(|s| {
            super::expanded_damage2026::sensor_operational(w, actor, &s.sensor).ok() == Some(true)
        })
        .filter_map(|s| w.db.sensors.get(&s.sensor))
        .filter_map(|d| super::aviation2026::operations_radar_profile(w, e, d))
        .filter(|p| p["role"] == "AI" || p["role"] == "LD/SD")
        .filter_map(|p| p["generation"].as_u64())
        .max()
        .map(|g| g as usize)
        .ok_or_else(|| "9.5 source AI/LD-SD radar generation missing".into())
}
/// §9.4.8 needs this aircraft's successful sensor provenance, never a fused
/// contact or fitted receiver alone. The one-tick bridge is procedural: source
/// scans run after commands, so a command consumes the preceding scan.
fn night_onboard_sensor(
    w: &World,
    actor: EntityId,
    target: EntityId,
    mr: f64,
    dogfight: bool,
) -> Result<Option<String>, String> {
    if !dogfight || w.day_night != crate::types::DayNight::Night {
        return Ok(None);
    }
    let policy = &tables()["night_dogfight"];
    let source = w.entity(actor).ok_or("9.4.8 own aircraft unavailable")?;
    let fresh = policy["command_scan_bridge_ticks"].as_i64().unwrap();
    let fitted = |sensor: &str| source.sensors.iter().any(|s| s.sensor == sensor);
    let sensor=w.rules.aaw.as_ref().into_iter().flat_map(|a|a.firecontrol.tracks.values()).filter(|t|t.observer==actor&&t.target==target&&t.last<=w.tick&&w.tick-t.last<=fresh).find_map(|track|{
        if !fitted(&track.sensor)||crate::sensors::sensor_damaged(source,&track.sensor)||super::expanded_damage2026::sensor_operational(w,actor,&track.sensor).ok()!=Some(true)||super::damage_runtime::marker(source)&&source.ext.get("damage.power_lost").copied().unwrap_or(0.)>0. {return None;}
        let def=w.db.sensors.get(&track.sensor)?;
        let qualified=match def.kind {
            crate::db::SensorKind::Radar=>source.systems.radar>0.05&&crate::sensors::radar_emitting(source)&&super::aviation2026::operations_radar_profile(w,source,def).is_some(),
            crate::db::SensorKind::Ir=>w.rules.detection.as_ref().and_then(|d|d.config.bindings.get(&format!("{}:{}",source.class,track.sensor))).is_some_and(|r|matches!(r,super::detection2026::Receiver::Ir {source,mode:super::detection2026::IrMode::Irst,..} if !source.trim().is_empty())),
            _=>false,
        };
        qualified.then(||track.sensor.clone())
    });
    if mr > policy["unrestricted_maneuver_rating_max"].as_f64().unwrap() && sensor.is_none() {
        return Err("9.4.8 night dogfight requires this aircraft's fresh operational radar or verified IRST detection; shared/visual/FLIR evidence does not qualify".into());
    }
    Ok(sensor)
}
/// §9.5.3 random lock. This is referee knowledge and is never emitted with hidden aircraft identity.
pub fn dogfight_target(
    w: &World,
    shooter: EntityId,
    target: EntityId,
    weapon: &str,
) -> Result<EntityId, String> {
    if super::aircombat2026::in_dogfight(w, shooter) {
        return Ok(target);
    }
    let Some(f) = w
        .rules
        .aircombat
        .as_ref()
        .and_then(|s| s.fights.values().find(|f| f.members.contains(&target)))
    else {
        return Ok(target);
    };
    let mut members: Vec<_> = f
        .members
        .iter()
        .copied()
        .filter(|id| w.entity(*id).is_some_and(|e| e.alive))
        .collect();
    members.sort();
    if members.is_empty() {
        return Err("9.5.3 no live aircraft in dogfight".into());
    }
    let mut rng = w.rng.derive(Rng::hash_tag(
        w.tick,
        shooter.0,
        &format!("9.5.3:{weapon}:{}", target.0),
    ));
    Ok(members[(rng.next_f64() * members.len() as f64) as usize])
}
pub fn set_hoj(
    w: &mut World,
    side: Side,
    unit: EntityId,
    weapon: &str,
    enabled: bool,
) -> Result<(), String> {
    let e = w
        .entity(unit)
        .filter(|e| e.side == side && e.alive)
        .ok_or("own aircraft absent")?;
    if !e.weapons.iter().any(|m| m.weapon == weapon) {
        return Err("weapon is not fitted".into());
    }
    if weapon_profile(weapon)?["hoj"] != true {
        return Err("7.4.2 Annex H4 does not verify HOJ mode for this missile variant".into());
    }
    w.entity_mut(unit)
        .unwrap()
        .ext
        .insert(format!("aam.hoj.{weapon}"), if enabled { 1. } else { 0. });
    super::aaw2026::trace(
        w,
        unit,
        "7.4.2.hoj_prelaunch",
        json!({"weapon":weapon,"enabled":enabled}),
    );
    Ok(())
}
pub fn fire_at(
    w: &mut World,
    shooter: EntityId,
    requested_target: EntityId,
    mount: usize,
    rounds: u8,
) -> Result<u8, String> {
    if !super::aaw2026::active(w) {
        return Err("source AAM requires explicit v4".into());
    }
    super::aircombat2026::release_allowed(w, shooter)?;
    let s = w
        .entity(shooter)
        .filter(|e| {
            e.alive
                && e.kind == PlatformKind::Aircraft
                && super::damage_runtime::weapons_operational(e)
        })
        .ok_or("AAM aircraft unavailable")?;
    let side = s.side;
    let m = s
        .weapons
        .get(mount)
        .filter(|m| m.mounts > 0 && m.ammo > 0)
        .ok_or("AAM mount empty")?
        .clone();
    super::aircombat2026::starting_weapon_gate(w, shooter, &m.weapon)?;
    let p = weapon_profile(&m.weapon)?;
    if !w.sides[side.index()]
        .contacts
        .values()
        .any(|c| c.about == requested_target && !c.stale && !c.bearing_only && c.quality >= 0.3)
        || super::detection2026::known_category(w, side, requested_target).is_none()
    {
        return Err("9.5 own usable classified contact required".into());
    }
    let state = &w.rules.aaw.as_ref().ok_or("AAW runtime absent")?.aam;
    if state
        .clocks
        .get(&shooter)
        .is_some_and(|last| w.tick < last + 60)
    {
        return Err("9.5 one AAM packet per aircraft per one-minute round".into());
    }
    let target = dogfight_target(w, shooter, requested_target, &m.weapon)?;
    let t = w
        .entity(target)
        .filter(|e| e.alive && e.kind == PlatformKind::Aircraft)
        .ok_or("9.5 source aircraft target data required")?;
    let remote_air = super::communications2026::received_air_solution(w, shooter, target)?;
    let target_position = remote_air.as_ref().map(|r| r.position).unwrap_or(t.pos);
    let target_altitude = remote_air
        .as_ref()
        .and_then(|r| r.altitude_m)
        .unwrap_or(t.altitude_m);
    let dogfight = super::aircombat2026::in_dogfight(w, shooter);
    if !super::aircombat2026::aam_position_allowed(
        w,
        shooter,
        p["aspect"].as_str().ok_or("source aspect missing")?,
    ) {
        return Err("9.4/9.5 dogfight attack position not obtained".into());
    }
    let radar_guided = p["guidance"].as_str().is_some_and(|g| g.contains("ARH"));
    let generation = radar(w, shooter, radar_guided)?;
    if radar_guided {
        let cap = aircraft_profile(&s.class)?["radar_targets"]
            .as_u64()
            .ok_or("6.3 simultaneous radar target fit unknown")? as usize;
        let targets: BTreeSet<_> = state
            .packets
            .values()
            .filter(|p| {
                p.shooter == shooter
                    && p.members
                        .iter()
                        .any(|id| w.entity(*id).is_some_and(|e| e.alive))
            })
            .map(|p| p.target)
            .collect();
        if !targets.contains(&target) && targets.len() >= cap {
            return Err("6.3 all verified AAM radar targets occupied".into());
        }
    }
    let (mr, maneuvering, signature, mut countermeasures) = target_data(
        w,
        target,
        shooter,
        p["seeker_generation"]
            .as_u64()
            .ok_or("source seeker generation missing")? as usize,
    )?;
    let night_sensor = night_onboard_sensor(w, shooter, target, mr, dogfight)?;
    if !radar_guided {
        countermeasures.jammer_generation = None;
    }
    // H4 supplies no SSkim exception for these selected variants; do not infer one from a generic alias.
    if target_altitude <= 20. {
        return Err("9.5 Annex H4 SSkim capability for this AAM variant not verified".into());
    }
    if target_altitude > n(&p["ceiling_m"])? {
        return Err("9.5 target above source AAM ceiling".into());
    }
    let base = n(&p[if dogfight {
        "boresight_range_nm"
    } else {
        "range_nm"
    }])?;
    let mut range = range_nm(
        base,
        s.speed_kts,
        s.altitude_m,
        t.speed_kts,
        angle_diff(t.course_deg, target_position.bearing_to(s.pos)).abs(),
        !maneuvering,
        dogfight,
    )?;
    if !radar_guided {
        let env = w
            .rules
            .detection
            .as_ref()
            .and_then(|d| d.config.environment.as_ref())
            .ok_or("7.4.2 IR AAM needs explicit precipitation/cloud environment")?;
        let factor = super::detection2026::tables()["ir"]["precipitation_multiplier"]
            [env.precipitation]
            .as_f64()
            .ok_or("IR precipitation outside source table")?;
        range *= factor;
        if let Some(cloud) = &env.cloud {
            if cloud.coverage_percent > 0
                && !((s.altitude_m < cloud.base_m && target_altitude < cloud.base_m)
                    || (s.altitude_m > cloud.top_m && target_altitude > cloud.top_m))
            {
                return Err(
                    "7.4.2 IR AAM cloud sightline needs verified clear gap before lock".into(),
                );
            }
        }
    }
    if s.pos.range_to(target_position) > range {
        return Err("9.5 outside modified source AAM range".into());
    }
    let hoj = s.ext.get(&format!("aam.hoj.{}", m.weapon)) == Some(&1.);
    if hoj && (p["hoj"] != true || !super::electronic_warfare::emitting(t)) {
        return Err("7.4.2 HOJ requires verified seeker and a radiating jammer target".into());
    }
    let count = rounds.min(2).min(m.ammo.min(2) as u8);
    if count == 0 {
        return Err("9.5 no AAM rounds requested".into());
    }
    let attack = Attack {
        ata: n(&p[if dogfight { "boresight_ata" } else { "ata" }])?,
        mr,
        maneuvering,
        speed: target_speed(w, target)?,
        signature,
        radar_generation: generation,
        snap_penalty: p["snap"] != true
            && super::operations::altitude_band(s.altitude_m)
                != super::operations::altitude_band(target_altitude),
        dogfight_unrated: dogfight && p["dogfight"] != true,
        countermeasures,
        hoj,
    };
    solution(&attack, count)?;
    let mut def =
        w.db.weapons
            .get(&m.weapon)
            .cloned()
            .ok_or("source AAM definition absent")?;
    def.range_nm = range;
    def.min_range_nm = 0.;
    def.speed_kts = n(&p["speed_kts"])?;
    let mut members = vec![];
    for _ in 0..count {
        if let Some(id) = crate::weapons::launch(
            w,
            shooter,
            &def,
            Some(target),
            remote_air.as_ref().map(|r| r.position),
        ) {
            w.entity_mut(id)
                .unwrap()
                .ext
                .insert("9.5.range_nm".into(), range);
            members.push(id)
        }
    }
    if members.len() != count as usize {
        for id in members {
            if let Some(e) = w.entity_mut(id) {
                e.alive = false;
                e.hp = 0.
            }
        }
        return Err("source AAM packet could not spawn atomically".into());
    }
    let identity_material = format!(
        "aam:{}:{}:{}:{}",
        shooter.0, m.weapon, target.0, members[0].0
    );
    // The complete packet state is referee-only. Its event identifier may be
    // shown to the shooter, so do not embed the hidden target's truth ID.
    let identity = format!(
        "aam:{:016x}",
        Rng::hash_tag(w.tick, shooter.0, &identity_material)
    );
    let packet = Packet {
        identity: identity.clone(),
        shooter,
        target,
        weapon: m.weapon.clone(),
        tick: w.tick,
        members: members.clone(),
        attack,
        guidance: p["guidance"].as_str().unwrap().into(),
        hoj,
        controller_evaded: false,
    };
    let state = &mut w.rules.aaw.as_mut().unwrap().aam;
    state.clocks.insert(shooter, w.tick);
    for id in &members {
        state.packets.insert(*id, packet.clone());
        state.evasion_queue.push((target, *id));
    }
    w.entity_mut(shooter).unwrap().weapons[mount].ammo -= count as u16;
    super::aviation2026::mark_under_fire(w, target);
    super::aaw2026::trace(
        w,
        shooter,
        "9.5.source_launch",
        json!({"identity":identity,"weapon":m.weapon,"rounds":count,"range_nm":range,"hoj":hoj,"random_lock_into_dogfight":target!=requested_target,"night_onboard_sensor":night_sensor,"rt_air_refs":remote_air.as_ref().map(|r|&r.refs),"nonmaneuvering_when_fired":!maneuvering}),
    );
    Ok(count)
}
pub fn terminal(w: &World, state: &mut State, id: EntityId) -> Result<Outcome, String> {
    let p = state
        .packets
        .get(&id)
        .ok_or("9.5 projectile has no committed source AAM packet")?
        .clone();
    if let Some(o) = state.outcomes.get(&p.identity) {
        return Ok(o.clone());
    }
    let mut attack = p.attack.clone();
    // A jettison or damage can change the current source MR and hardware. The
    // launch attack is also the persisted 9.5.1 declaration: a plane declared
    // surprised/nonmaneuvering when fired upon keeps MR0 and half eligible
    // countermeasures, even if that declaration window ends during flight.
    let (mr, maneuvering, signature, cm) = target_data(
        w,
        p.target,
        p.shooter,
        attack.countermeasures.seeker_generation,
    )?;
    attack.mr = if p.attack.maneuvering { mr } else { 0. };
    attack.maneuvering = p.attack.maneuvering && maneuvering;
    attack.signature = signature;
    attack.speed = target_speed(w, p.target)?;
    attack.countermeasures = cm;
    if !p.guidance.contains("ARH") {
        attack.countermeasures.jammer_generation = None;
    }
    let mut result = solution(&attack, p.members.len() as u8)?;
    if (p.controller_evaded || super::aircombat2026::controlling_missiles_stupid(w, p.shooter))
        && p.guidance != "TARH"
    {
        result.probability = 0.;
        result
            .modifiers
            .insert("forced_miss_controller_evading".into(), 1.);
    }
    if p.hoj
        && !w
            .entity(p.target)
            .is_some_and(super::electronic_warfare::emitting)
    {
        result.probability = 0.;
        result
            .modifiers
            .insert("forced_miss_jammer_silent".into(), 1.);
    }
    let mut rng = w
        .rng
        .derive(Rng::hash_tag(p.tick, p.shooter.0, &p.identity));
    let d100 = 1 + (rng.next_f64() * 100.) as u8;
    let o = Outcome {
        identity: p.identity.clone(),
        d100,
        probability: result.probability,
        hit: d100 as f64 <= result.probability * 100.,
        consumed_projectiles: p.members.clone(),
        solution: result,
    };
    state.outcomes.insert(p.identity, o.clone());
    Ok(o)
}
/// Commit every same-tick launch before evasion prohibits subsequent releases.
pub fn after_fire_scan(w: &mut World) {
    if !super::aaw2026::active(w) {
        return;
    }
    let queue = std::mem::take(&mut w.rules.aaw.as_mut().unwrap().aam.evasion_queue);
    for (target, missile) in queue {
        super::aircombat2026::begin_aam_evasion(w, target, missile)
    }
}
pub fn configure_database(db: &mut crate::Database) {
    for (id, p) in tables()["weapons"].as_object().unwrap() {
        if p["ceiling_m"].is_null() {
            continue;
        }
        let d = WeaponDef {
            id: id.clone(),
            name: p["name"].as_str().unwrap().into(),
            kind: WeaponKind::Aam,
            targets: crate::db::Targets {
                air: true,
                missile: true,
                ..Default::default()
            },
            range_nm: n(&p["range_nm"]).unwrap(),
            min_range_nm: 0.,
            speed_kts: n(&p["speed_kts"]).unwrap(),
            enable_range_nm: 0.,
            pk: 0.,
            salvo: 2,
            reload_s: 0.,
            damage: 0.,
            guidance: Guidance::HomingAir,
            ir: !p["guidance"].as_str().unwrap().contains("ARH"),
            directors: 0,
            rule_data: None,
        };
        db.weapons.insert(id.clone(), d);
    }
    for class in ["us_f18e", "us_f18e_asm", "us_f35c"] {
        if let Some(p) = db.platforms.get_mut(class) {
            for m in &mut p.weapons {
                if m.weapon == "us_aim120" {
                    m.weapon = "us_aim120d".into()
                }
                if m.weapon == "us_aim9x" {
                    m.weapon = "us_aim9x2".into()
                }
            }
        }
    }
    if let Some(p) = db.platforms.get_mut("cn_j15") {
        for m in &mut p.weapons {
            if m.weapon == "cn_pl8" {
                m.weapon = "cn_pl8b".into()
            }
        }
    }
}

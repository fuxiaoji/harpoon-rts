//! ASW depth: sound-layer propagation, towed-array state, sonobuoys, MAD,
//! diesel-battery endurance.
//!
//! Rules sources (`14-2.HpnV_Rules_1_Jan_26.pdf`):
//! - §5.4.2 Towed Arrays, pp. 5-8..5-9 (PDF 47-48): streaming/retrieval
//!   takes seven Tactical Turns (21 min) and is limited to 15 kt while the
//!   array is being streamed; once deployed the safe towing speed applies.
//! - §5.4.4 Sonobuoys, p. 5-9 (PDF 48): passive buoy life 1-8 hours.
//! - §5.4.6.1/5.4.6.2 Active/Passive Sonar Detection, pp. 5-13..5-14
//!   (PDF 52-53); Layer Modifiers table p. 5-10 (PDF 49): a sonar above the
//!   Layer suffers -5 (range ×0.30) against a contact below it.
//! - §5.4.7.1 Direct Path and "The Layer", pp. 5-17..5-18 (PDF 56-57);
//!   Layer depth table p. 5-10: Shallow 25 m, Moderate 50 m, Deep 100 m.
//! - §5.4.6.3 Sonobuoy Detection, pp. 5-14..5-15 (PDF 53-54).
//! - §5.7.1 MAD, p. 5-21 (PDF 60): low-altitude straight/level pass,
//!   400-700 yd; Periscope/Shallow through Intermediate V only (Deep+ is
//!   undetectable), with depth-band modifiers. The engine's simplified
//!   `SensorKind::Magnetic` branch in `sensors.rs` cuts off below 60 m and
//!   applies a fixed 0.5x range factor; no extra step logic is added here.
//! - §3.2.4 Diesel Submarine Battery Endurance, p. 3-7 (PDF 29) and the
//!   Battery Discharge Rates table on p. 3-8 (PDF 30).
//!
//! Hooks:
//! - `propagation_modifier(observer, target)` — multiplies sonar effective
//!   ranges inside the sensor pass (layer physics + towed-array state).
//! - `step(world)` — towed-array speed automation, diesel battery
//!   drain/regen, and sonobuoy laying/expiry.
//!
//! Chapter 10 (Submarine Warfare) governs sub-launched weapons; the ASW
//! state requested for this module is sourced from §§5.4, 5.7 and 3.2.4,
//! with no additional chapter-10 state added here.

use crate::db::Database;
use crate::events::{EventKind, Visibility};
use crate::geom::Vec2;
use crate::types::{ContactId, EntityId, PlatformKind, Side};
use crate::world::{Entity, MissionState, SensorInstance, World};
use std::collections::BTreeMap;

// --- Acoustic propagation constants -------------------------------------

/// Approximate depth of the deep thermocline ("The Layer").
///
/// SIMPLIFIED: §5.4.7.1 and the Layer table (p. 5-10) describe Shallow /
/// Moderate / Deep layers whose nominal depths are 25/50/100 m and whose
/// exact depth comes from the scenario environment. The engine has no
/// environment-layer state, so 150 m is used as the boundary for a target
/// in the "deep" layer / below the thermocline.
const THERMOCLINE_DEPTH_M: f64 = 150.0;

/// Below-layer range multiplier. The official Layer table gives -5, which
/// maps to a ×0.30 range modifier; this module uses ×0.5 (SIMPLIFIED) so
/// the layer penalty does not multiply the engine's existing speed/noise
/// multiplier chain down to an unusable value.
const CROSS_LAYER_FACTOR: f64 = 0.5;

/// A submarine hugging the bottom is harder to detect (bottom masking /
/// bottom-limited direct path). No direct table entry exists in §5.4.7.2,
/// which covers active bottom-bounce propagation instead; 0.8 is an
/// ESTIMATED adaptation for a target deeper than 200 m.
const BOTTOM_DEPTH_M: f64 = 200.0;
const BOTTOM_FACTOR: f64 = 0.8;

// --- Towed-array constants ----------------------------------------------

const TOWED_SENSOR_PREFIX: &str = "sonar_p_tow";
const EXT_TOW_DEPLOYED: &str = "tow_deployed";
const EXT_TOW_TIMER: &str = "asw.tow_timer";
/// SIMPLIFIED/ADAPTED: §5.4.2 says 21 minutes to stream/retrieve and 15 kt
/// during streaming. The work order specifies real-time RTS automation:
/// automatic recovery above 25 kt and deployment 30 s after slowing below
/// 20 kt.
const TOW_RETRACT_KTS: f64 = 25.0;
const TOW_DEPLOY_KTS: f64 = 20.0;
const TOW_DEPLOY_DELAY_S: f64 = 30.0;
/// A retracted or still-streaming array is half as effective.
const TOW_NOT_DEPLOYED_FACTOR: f64 = 0.5;

// --- Diesel battery constants -------------------------------------------

const EXT_BATTERY: &str = "battery";
const EXT_BATTERY_LOW_ANNOUNCED: &str = "asw.battery_low_announced";
const BATTERY_FULL: f64 = 100.0;
const BATTERY_LOW: f64 = 20.0;
/// SIMPLIFIED: §3.2.4 discharges by speed from the Battery Discharge Rate
/// table (e.g. 0.2 charge units per Tactical Turn at 4 kt, 8.4 at 20 kt).
/// The work order collapses this to a constant submerged drain and a
/// constant snorkel charge; `battery` is normalised to 0..100.
const BATTERY_DRAIN_PER_S: f64 = 0.05;
const BATTERY_CHARGE_PER_S: f64 = 0.1;
const SNORKEL_DEPTH_M: f64 = 20.0;

// --- Sonobuoy constants --------------------------------------------------

const SONOBUOY_CLASS: &str = "sonobuoy";
const SONOBUOY_SENSOR: &str = "sonar_p_buoy";
const EXT_EXPIRES_AT: &str = "expires_at";
/// Remaining seconds until the next buoy from this aircraft (RTS cadence).
const EXT_BUOY_TIMER: &str = "asw.buoy_timer";
/// §5.4.4: passive sonobuoys live one to eight hours; 8 h is the maximum.
const SONOBUOY_LIFETIME_S: i64 = 28_800;
const SONOBUOY_INTERVAL_S: i64 = 120;
/// The drop requires a tracked subsurface contact. `quality > 0.4` per the
/// work order (strictly greater, not >=).
const SONOBUOY_MIN_QUALITY: f64 = 0.4;
/// §5.4.4 requires a low/medium-altitude pass at 325 kt or less. The RTS
/// adaptation uses a stricter hover profile (air::asw_station places the
/// helo at 30 m and zero speed).
const SONOBUOY_HOVER_SPEED_KTS: f64 = 1.0;
const SONOBUOY_HOVER_ALT_M: f64 = 100.0;

/// Extra DB entries: the passive sonobuoy platform and its 5 nm passive set.
///
/// §5.4.4 (PDF 48) describes passive sonobuoys with one-to-eight-hour lives;
/// the exact detection range is an Annex K2 value. The work order fixes the
/// adaptation at 5 nm.
pub fn db() -> Database {
    const ASW_DB_JSON: &str = r#"{
      "platforms": {
        "sonobuoy": {
          "id": "sonobuoy", "name": "Sonobuoy", "kind": "Ship",
          "hp": 1, "max_speed_kts": 0, "turn_rate_deg_s": 0,
          "rcs": "VerySmall", "noise_db": 0, "vp_value": 0,
          "endurance_s": 28800,
          "sensors": ["sonar_p_buoy"],
          "weapons": []
        }
      },
      "weapons": {},
      "sensors": {
        "sonar_p_buoy": {
          "id": "sonar_p_buoy", "name": "Passive Sonobuoy",
          "kind": "SonarPassive", "alt_m": 0,
          "sub_range_nm": 5
        }
      }
    }"#;
    serde_json::from_str(ASW_DB_JSON).expect("ASW database extras must parse")
}

/// Sonar propagation multiplier applied to sonar effective range.
///
/// Layer model (SIMPLIFIED): a target submarine deeper than
/// [`THERMOCLINE_DEPTH_M`] is treated as being below the Layer. If the
/// observer is a submarine also below that boundary, both are in the same
/// layer and there is no cross-layer loss; every other observer (surface
/// ship, aircraft, sonobuoy, shallow submarine) counts as above the Layer
/// and incurs ×0.5. A target deeper than [`BOTTOM_DEPTH_M`] additionally
/// incurs the estimated bottom factor ×0.8.
///
/// Note: the hook receives the whole observer, not the particular sensor
/// (sensors.rs calls it from `effective_range`). A platform carrying a
/// towed array therefore applies the retracted-array penalty to all of its
/// sonar ranges. For the built-in FFG this also attenuates its active hull
/// sonar while the array is stowed; that coupling is the documented
/// simplification of the per-observer hook. Likewise, a deployed towed
/// array is not given its own below-Layer depth (§5.4.2 streams it one
/// depth band below the Layer), so a surface observer still pays the
/// cross-layer factor against a deep target.
pub fn propagation_modifier(observer: &Entity, target: &Entity) -> f64 {
    let mut m = 1.0;
    if target.kind == PlatformKind::Sub {
        let target_below_layer = target.depth_m > THERMOCLINE_DEPTH_M;
        let observer_below_layer =
            observer.kind == PlatformKind::Sub && observer.depth_m > THERMOCLINE_DEPTH_M;
        if target_below_layer && !observer_below_layer {
            m *= CROSS_LAYER_FACTOR;
        }
        if target.depth_m > BOTTOM_DEPTH_M {
            m *= BOTTOM_FACTOR;
        }
    }
    if has_towed_array(observer) && !tow_deployed(observer) {
        m *= TOW_NOT_DEPLOYED_FACTOR;
    }
    m
}

/// Per-tick ASW stateful behavior.
pub fn step(world: &mut World) {
    update_towed_arrays(world);
    update_diesel_batteries(world);
    expire_sonobuoys(world);
    deploy_sonobuoys(world);
}

// ---------------------------------------------------------------------------
// Towed arrays
// ---------------------------------------------------------------------------

fn has_towed_array(e: &Entity) -> bool {
    e.sensors
        .iter()
        .any(|s| s.sensor.starts_with(TOWED_SENSOR_PREFIX))
}

fn tow_deployed(e: &Entity) -> bool {
    e.ext.get(EXT_TOW_DEPLOYED).copied().unwrap_or(0.0) >= 0.5
}

/// Auto-manage the towed array: recover above 25 kt, stream after 30 s
/// continuously below 20 kt. The timer is reset whenever the speed is
/// outside the deployment band or the array is already deployed.
fn update_towed_arrays(world: &mut World) {
    let ids: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| e.alive && has_towed_array(e))
        .map(|e| e.id)
        .collect();

    let mut transitions: Vec<(EntityId, Side, bool, f64)> = Vec::new();
    for id in ids {
        let Some(e) = world.entity_mut(id) else { continue };
        let was = tow_deployed(e);
        let speed = e.speed_kts;
        let mut timer = e.ext.get(EXT_TOW_TIMER).copied().unwrap_or(0.0);
        let mut next = was;

        if speed > TOW_RETRACT_KTS {
            next = false;
            timer = 0.0;
        } else if !was && speed < TOW_DEPLOY_KTS {
            if timer <= 0.0 {
                timer = TOW_DEPLOY_DELAY_S;
            }
            timer -= crate::world::TICK_SECONDS;
            if timer <= 0.0 {
                next = true;
                timer = 0.0;
            }
        } else if !was {
            // Speed in the 20-25 kt band: the 30 s deployment run is broken.
            timer = 0.0;
        }

        e.ext
            .insert(EXT_TOW_DEPLOYED.to_string(), if next { 1.0 } else { 0.0 });
        e.ext.insert(EXT_TOW_TIMER.to_string(), timer);
        if next != was {
            transitions.push((id, e.side, next, speed));
        }
    }

    for (id, side, deployed, speed) in transitions {
        let (rule, detail) = if deployed {
            (
                "towed_array_deployed",
                format!(
                    "towed array streamed after {TOW_DEPLOY_DELAY_S:.0} s below \
                     {TOW_DEPLOY_KTS:.0} kt (speed {speed:.1} kt)"
                ),
            )
        } else {
            (
                "towed_array_retracted",
                format!(
                    "towed array recovered: speed {speed:.1} kt above \
                     {TOW_RETRACT_KTS:.0} kt"
                ),
            )
        };
        world.emit(
            EventKind::RulesEvent {
                domain: "asw".into(),
                rule: rule.into(),
                side: Some(side),
                unit: Some(id),
                contact: None,
                detail,
            },
            Visibility::Side(side),
        );
    }
}

// ---------------------------------------------------------------------------
// Diesel-electric battery
// ---------------------------------------------------------------------------

fn is_diesel_sub(e: &Entity, db: &Database) -> bool {
    if e.kind != PlatformKind::Sub {
        return false;
    }
    let id = e.class.to_ascii_lowercase();
    if id.starts_with("ssk") || id == "ss" {
        return true;
    }
    db.platforms
        .get(&e.class)
        .map(|c| {
            let name = c.name.to_ascii_lowercase();
            name.contains("diesel") || name.contains("conventional") || name.contains("aip")
        })
        .unwrap_or(false)
}

/// Battery drain/charge and the low-battery speed restriction.
///
/// `battery` is 100 at start, drains 0.05/s at depth > 20 m and charges
/// 0.1/s (capped at 100) at snorkel depth (<= 20 m), per the work order.
/// Below 20 the boat's speed and its standing speed order are clamped to
/// half of its current maximum speed; a `battery_low` RulesEvent is emitted
/// once per boat.
fn update_diesel_batteries(world: &mut World) {
    let ids: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| e.alive && is_diesel_sub(e, &world.db))
        .map(|e| e.id)
        .collect();

    let mut low_events: Vec<(EntityId, Side, f64, f64)> = Vec::new();
    for id in ids {
        // Resolve the cap before taking the mutable entity borrow.
        let half_max = world.entities[&id].max_speed(&world.db) * 0.5;
        let Some(e) = world.entity_mut(id) else { continue };

        let mut battery = e
            .ext
            .get(EXT_BATTERY)
            .copied()
            .unwrap_or(BATTERY_FULL)
            .clamp(0.0, BATTERY_FULL);
        if e.depth_m > SNORKEL_DEPTH_M {
            battery = (battery - BATTERY_DRAIN_PER_S).max(0.0);
        } else {
            battery = (battery + BATTERY_CHARGE_PER_S).min(BATTERY_FULL);
        }
        e.ext.insert(EXT_BATTERY.to_string(), battery);

        if battery < BATTERY_LOW {
            if e.speed_kts > half_max {
                e.speed_kts = half_max;
            }
            if e.ordered_speed_kts > half_max {
                e.ordered_speed_kts = half_max;
            }
            let announced = e
                .ext
                .get(EXT_BATTERY_LOW_ANNOUNCED)
                .copied()
                .unwrap_or(0.0)
                >= 0.5;
            if !announced {
                e.ext
                    .insert(EXT_BATTERY_LOW_ANNOUNCED.to_string(), 1.0);
                low_events.push((id, e.side, battery, half_max));
            }
        }
    }

    for (id, side, battery, half_max) in low_events {
        world.emit(
            EventKind::RulesEvent {
                domain: "asw".into(),
                rule: "battery_low".into(),
                side: Some(side),
                unit: Some(id),
                contact: None,
                detail: format!(
                    "battery {battery:.1}% below {BATTERY_LOW:.0}%; max speed halved \
                     to {half_max:.1} kt"
                ),
            },
            Visibility::Side(side),
        );
    }
}

// ---------------------------------------------------------------------------
// Sonobuoys
// ---------------------------------------------------------------------------

fn is_hovering_asw(e: &Entity) -> bool {
    matches!(e.mission, Some(MissionState::Asw { .. }))
        && e.speed_kts < SONOBUOY_HOVER_SPEED_KTS
        && e.altitude_m < SONOBUOY_HOVER_ALT_M
}

/// Highest-quality live subsurface contact for `side`, if it exceeds the
/// drop threshold. Ties resolve to the lowest contact id for determinism.
fn best_sub_contact(world: &World, side: Side) -> Option<(f64, ContactId, Vec2)> {
    world.sides[side.index()]
        .contacts
        .values()
        .filter(|c| !c.stale && c.quality > SONOBUOY_MIN_QUALITY)
        .filter_map(|c| {
            let target = world.entities.get(&c.about)?;
            if !target.alive || target.kind != PlatformKind::Sub {
                return None;
            }
            Some((c.quality, c.id, c.estimate_at(world.tick)))
        })
        .max_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.1.cmp(&a.1))
        })
}

/// Lay one buoy every 120 s from each hovering, ASW-tasked aircraft that has
/// a tracked submarine contact (quality > 0.4). The buoy is placed on the
/// contact estimate and expires after eight hours (§5.4.4).
fn deploy_sonobuoys(world: &mut World) {
    let now = world.tick;
    let air_ids: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| e.alive && e.kind == PlatformKind::Aircraft && is_hovering_asw(e))
        .map(|e| e.id)
        .collect();

    struct Drop {
        aircraft: EntityId,
        side: Side,
        contact: ContactId,
        quality: f64,
        pos: Vec2,
    }
    let mut drops: Vec<Drop> = Vec::new();

    for id in air_ids {
        let side = world.entities[&id].side;
        let Some((quality, contact, pos)) = best_sub_contact(world, side) else {
            continue;
        };

        // Countdown while the aircraft is eligible: the first eligible tick
        // starts the 120 s cadence (no immediate drop), then one buoy per
        // 120 s.
        let mut timer = world.entities[&id]
            .ext
            .get(EXT_BUOY_TIMER)
            .copied()
            .unwrap_or(SONOBUOY_INTERVAL_S as f64);
        timer -= crate::world::TICK_SECONDS;
        let lay = timer <= 0.0;
        if lay {
            timer = SONOBUOY_INTERVAL_S as f64;
        }
        if let Some(e) = world.entity_mut(id) {
            e.ext.insert(EXT_BUOY_TIMER.to_string(), timer);
        }
        if lay {
            drops.push(Drop {
                aircraft: id,
                side,
                contact,
                quality,
                pos,
            });
        }
    }

    for drop in drops {
        let Some(class) = world.db.platforms.get(SONOBUOY_CLASS).cloned() else {
            continue;
        };
        if !class.sensors.iter().any(|s| s == SONOBUOY_SENSOR) {
            continue;
        }
        let id = world.allocate_id();
        let name = world.auto_name(drop.side, &class.name);
        let sensors: Vec<SensorInstance> = class
            .sensors
            .iter()
            .map(|s| SensorInstance {
                sensor: s.clone(),
                active: false,
            })
            .collect();
        // The class carries the eight-hour passive life in `endurance_s`
        // (Aircraft-only in normal platform data, reused here as the buoy's
        // selectable lifetime from §5.4.4).
        let lifetime_s = if class.endurance_s > 0.0 {
            class.endurance_s as i64
        } else {
            SONOBUOY_LIFETIME_S
        };
        let mut ext: BTreeMap<String, f64> = BTreeMap::new();
        ext.insert(EXT_EXPIRES_AT.to_string(), (now + lifetime_s) as f64);

        let buoy = Entity {
            id,
            side: drop.side,
            class: class.id.clone(),
            name,
            kind: class.kind,
            pos: drop.pos,
            course_deg: 0.0,
            speed_kts: 0.0,
            ordered_course_deg: 0.0,
            ordered_speed_kts: 0.0,
            waypoints: Default::default(),
            altitude_m: 0.0,
            ordered_altitude_m: 0.0,
            depth_m: 0.0,
            ordered_depth_m: 0.0,
            hp: class.hp,
            hp_max: class.hp,
            alive: true,
            systems: Default::default(),
            fire_on: false,
            assigned: None,
            weapons: Vec::new(),
            sensors,
            chaff: 0,
            decoys: 0,
            auto_chaff: false,
            chaff_active_s: 0.0,
            endurance_s: None,
            burn_s: 0.0,
            flooding_s: 0.0,
            ext,
            deck: None,
            mission: None,
            station: None,
            projectile: None,
            last_emit_tick: now,
        };
        world.entities.insert(id, buoy);

        world.emit(
            EventKind::RulesEvent {
                domain: "asw".into(),
                rule: "sonobuoy_laid".into(),
                side: Some(drop.side),
                unit: Some(drop.aircraft),
                contact: Some(drop.contact),
                detail: format!(
                    "sonobuoy #{id} laid at ({:.1}, {:.1}) on contact {} \
                     quality {:.2}; expires t={}",
                    drop.pos.x,
                    drop.pos.y,
                    drop.contact,
                    drop.quality,
                    now + lifetime_s
                ),
            },
            Visibility::Side(drop.side),
        );
    }
}

/// Remove sonobuoys whose eight-hour soluble plug has expired (§5.4.4).
fn expire_sonobuoys(world: &mut World) {
    let now = world.tick;
    let expired: Vec<(EntityId, Side, Vec2)> = world
        .entities
        .values()
        .filter(|e| {
            e.class == SONOBUOY_CLASS
                && e.ext
                    .get(EXT_EXPIRES_AT)
                    .map(|t| now as f64 >= *t)
                    .unwrap_or(false)
        })
        .map(|e| (e.id, e.side, e.pos))
        .collect();

    for (id, side, pos) in expired {
        world.entities.remove(&id);
        world.emit(
            EventKind::RulesEvent {
                domain: "asw".into(),
                rule: "sonobuoy_expired".into(),
                side: Some(side),
                unit: Some(id),
                contact: None,
                detail: format!(
                    "sonobuoy #{id} expired after {SONOBUOY_LIFETIME_S} s at \
                     ({:.1}, {:.1})",
                    pos.x, pos.y
                ),
            },
            Visibility::Side(side),
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{SensorDef, SensorKind};
    use crate::scenario::{load_scenario, Scenario, UnitSpawn};
    use crate::types::{DayNight, PlatformKind};
    use crate::world::{Contact, Weather};

    fn scenario(units: Vec<UnitSpawn>) -> Scenario {
        Scenario {
            id: "asw-test".into(),
            name: "ASW test".into(),
            description: String::new(),
            context: Default::default(),
            duration_s: 40_000,
            weather: Weather { visibility_nm: 12.0, jam: [0.0, 0.0], overcast: 0.0, rain: 0.0 },
            day_night: DayNight::Day,
            seed: 7,
            victory: Default::default(),
            units,
        }
    }

    fn world(units: Vec<UnitSpawn>) -> World {
        load_scenario(scenario(units), Database::built_in()).unwrap()
    }

    fn unit(side: Side, class: &str, name: &str, x: f64, y: f64, speed: f64) -> UnitSpawn {
        UnitSpawn {
            side,
            class: class.into(),
            name: Some(name.into()),
            pos: Vec2::new(x, y),
            course_deg: 0.0,
            speed_kts: speed,
            depth_m: None,
            altitude_m: None,
            waypoints: None,
            follow: None,
            sonar_active: false,
            fire_on: false,
        }
    }

    fn sub(side: Side, name: &str, x: f64, y: f64, depth_m: f64) -> UnitSpawn {
        UnitSpawn {
            depth_m: Some(depth_m),
            ..unit(side, "ssk", name, x, y, 0.0)
        }
    }

    fn air(side: Side, name: &str, x: f64, y: f64, altitude_m: f64) -> UnitSpawn {
        UnitSpawn {
            altitude_m: Some(altitude_m),
            ..unit(side, "helo_asw", name, x, y, 0.0)
        }
    }

    fn id_of(w: &World, name: &str) -> EntityId {
        w.entities
            .values()
            .find(|e| e.name == name)
            .map(|e| e.id)
            .unwrap_or_else(|| panic!("no entity named {name}"))
    }

    fn ticks(w: &mut World, n: i64) {
        for _ in 0..n {
            crate::sim::tick(w, &[]);
        }
    }

    fn buoys(w: &World) -> usize {
        w.entities.values().filter(|e| e.class == SONOBUOY_CLASS).count()
    }

    fn seed_sub_contact(
        w: &mut World,
        side: Side,
        about: EntityId,
        pos: Vec2,
        quality: f64,
    ) -> ContactId {
        let state = &mut w.sides[side.index()];
        state.next_contact += 1;
        let cid = ContactId(state.next_contact);
        state.contacts.insert(
            cid,
            Contact {
                id: cid,
                about,
                category: PlatformKind::Sub,
                class_name: None,
                estimate: pos,
                est_course_deg: 0.0,
                est_speed_kts: 0.0,
                bearing_only: false,
                quality,
                last_update_tick: 0,
                first_tick: 0,
                stale: false,
                sensors: vec!["seeded".into()],
                offset: Vec2::zero(),
                announced: false,
            },
        );
        cid
    }

    fn rules_event_count(w: &World, rule: &str) -> usize {
        w.events
            .iter()
            .filter(|e| {
                matches!(
                    &e.kind,
                    EventKind::RulesEvent { domain, rule: r, .. }
                        if domain == "asw" && r == rule
                )
            })
            .count()
    }

    #[test]
    fn layer_propagation_crosses_and_bottom_factor() {
        // DDG has no towed array, so only layer physics affect the result.
        let w = world(vec![
            unit(Side::Blue, "ddg_aegis", "BLUE DDG", 0.0, 0.0, 0.0),
            sub(Side::Blue, "BLUE SUB", 0.0, 10.0, 180.0),
            sub(Side::Red, "RED SUB", 0.0, 20.0, 60.0),
        ]);
        let ship = w.entity(id_of(&w, "BLUE DDG")).unwrap().clone();
        let mut blue_sub = w.entity(id_of(&w, "BLUE SUB")).unwrap().clone();
        let mut red_sub = w.entity(id_of(&w, "RED SUB")).unwrap().clone();

        // Target above the layer: no loss from any above-layer observer.
        assert!((propagation_modifier(&ship, &red_sub) - 1.0).abs() < 1e-12);

        // Surface observer vs below-layer target: ×0.5.
        red_sub.depth_m = 180.0;
        assert!((propagation_modifier(&ship, &red_sub) - 0.5).abs() < 1e-12);

        // The bottom-adjacent target adds the estimated ×0.8 term.
        red_sub.depth_m = 210.0;
        assert!((propagation_modifier(&ship, &red_sub) - 0.4).abs() < 1e-12);

        // A shallow (above-layer) submarine observer sees the same ×0.5.
        blue_sub.depth_m = 60.0;
        red_sub.depth_m = 180.0;
        assert!((propagation_modifier(&blue_sub, &red_sub) - 0.5).abs() < 1e-12);

        // Both below the layer: same-layer propagation is ×1.0.
        blue_sub.depth_m = 180.0;
        assert!((propagation_modifier(&blue_sub, &red_sub) - 1.0).abs() < 1e-12);

        // Both bottomed: only the ×0.8 bottom term remains.
        blue_sub.depth_m = 210.0;
        red_sub.depth_m = 210.0;
        assert!((propagation_modifier(&blue_sub, &red_sub) - 0.8).abs() < 1e-12);
    }

    #[test]
    fn towed_array_halves_until_automatically_deployed() {
        let mut w = world(vec![
            unit(Side::Blue, "ffg", "BLUE FFG", 0.0, 0.0, 18.0),
            sub(Side::Red, "RED SUB", 0.0, 20.0, 60.0),
        ]);
        let ffg = id_of(&w, "BLUE FFG");
        let red = id_of(&w, "RED SUB");

        // The FFG carries the towed array; it starts retracted (ext absent).
        assert!(w
            .entity(ffg)
            .unwrap()
            .sensors
            .iter()
            .any(|s| s.sensor.starts_with(TOWED_SENSOR_PREFIX)));
        assert!((propagation_modifier(w.entity(ffg).unwrap(), w.entity(red).unwrap()) - 0.5).abs() < 1e-12);

        // 30 s below 20 kt: still retracted after 29 ticks, deployed on the
        // 30th.
        ticks(&mut w, 29);
        assert!(!tow_deployed(w.entity(ffg).unwrap()));
        ticks(&mut w, 1);
        assert!(tow_deployed(w.entity(ffg).unwrap()));
        assert!((propagation_modifier(w.entity(ffg).unwrap(), w.entity(red).unwrap()) - 1.0).abs() < 1e-12);
        assert_eq!(rules_event_count(&w, "towed_array_deployed"), 1);

        // Speed above 25 kt recovers the array immediately.
        {
            let e = w.entity_mut(ffg).unwrap();
            e.speed_kts = 26.0;
            e.ordered_speed_kts = 26.0;
        }
        ticks(&mut w, 1);
        assert!(!tow_deployed(w.entity(ffg).unwrap()));
        assert_eq!(rules_event_count(&w, "towed_array_retracted"), 1);

        // Below 20 kt again: 30 s timer, then deploy.
        {
            let e = w.entity_mut(ffg).unwrap();
            e.speed_kts = 18.0;
            e.ordered_speed_kts = 18.0;
        }
        ticks(&mut w, 29);
        assert!(!tow_deployed(w.entity(ffg).unwrap()));
        ticks(&mut w, 1);
        assert!(tow_deployed(w.entity(ffg).unwrap()));
    }

    #[test]
    fn diesel_battery_drains_charges_and_limits_speed_once() {
        let mut w = world(vec![sub(Side::Red, "RED SUB", 0.0, 0.0, 30.0)]);
        let id = id_of(&w, "RED SUB");
        {
            let e = w.entity_mut(id).unwrap();
            e.speed_kts = 20.0;
            e.ordered_speed_kts = 20.0;
        }

        for _ in 0..100 {
            step(&mut w);
        }
        let battery = w.entity(id).unwrap().ext[EXT_BATTERY];
        assert!((battery - 95.0).abs() < 1e-9, "battery {battery}");

        // Snorkel depth: charge at 0.1/s, capped at 100.
        {
            let e = w.entity_mut(id).unwrap();
            e.depth_m = 15.0;
            e.ordered_depth_m = 15.0;
        }
        for _ in 0..100 {
            step(&mut w);
        }
        assert_eq!(w.entity(id).unwrap().ext[EXT_BATTERY], BATTERY_FULL);

        // Drop below 20: speed and standing order are clamped to half max.
        {
            let e = w.entity_mut(id).unwrap();
            e.depth_m = 30.0;
            e.ordered_depth_m = 30.0;
            e.ext.insert(EXT_BATTERY.to_string(), 25.0);
            e.speed_kts = 20.0;
            e.ordered_speed_kts = 20.0;
        }
        for _ in 0..101 {
            step(&mut w);
        }
        let battery = w.entity(id).unwrap().ext[EXT_BATTERY];
        assert!(battery < BATTERY_LOW, "battery {battery}");
        assert!((w.entity(id).unwrap().speed_kts - 10.0).abs() < 1e-9);
        assert!((w.entity(id).unwrap().ordered_speed_kts - 10.0).abs() < 1e-9);
        assert_eq!(rules_event_count(&w, "battery_low"), 1);

        // Still low: no duplicate announcement.
        for _ in 0..10 {
            step(&mut w);
        }
        assert_eq!(rules_event_count(&w, "battery_low"), 1);
    }

    #[test]
    fn sonobuoy_db_matches_work_order() {
        let db = Database::built_in();
        let buoy = db.platforms.get(SONOBUOY_CLASS).expect("sonobuoy platform");
        assert_eq!(buoy.kind, PlatformKind::Ship);
        assert_eq!(buoy.hp, 1.0);
        assert_eq!(buoy.noise_db, 0.0);
        assert_eq!(buoy.endurance_s, SONOBUOY_LIFETIME_S as f64);
        assert!(buoy.sensors.iter().any(|s| s == SONOBUOY_SENSOR));
        let sensor = db.sensors.get(SONOBUOY_SENSOR).expect("sonobuoy sensor");
        assert_eq!(sensor.kind, SensorKind::SonarPassive);
        assert_eq!(sensor.sub_range_nm, 5.0);
    }

    #[test]
    fn sonobuoy_predicts_passive_contact_in_engine() {
        // A pre-placed buoy is a normal Ship observer with the 5 nm passive
        // set; sensors.rs applies its speed/noise multipliers on top.
        let mut w = world(vec![
            unit(Side::Blue, "sonobuoy", "BLUE BUOY", 0.0, 0.0, 0.0),
            sub(Side::Red, "NEAR SUB", 0.2, 0.0, 30.0),
            sub(Side::Red, "FAR SUB", 3.0, 0.0, 30.0),
        ]);
        ticks(&mut w, 4);

        let near = id_of(&w, "NEAR SUB");
        let far = id_of(&w, "FAR SUB");
        let near_contact = w
            .sides[Side::Blue.index()]
            .contacts
            .values()
            .find(|c| c.about == near)
            .expect("buoy should detect the 0.2 nm sub");
        assert!(near_contact.sensors.iter().any(|s| s == SONOBUOY_SENSOR));
        assert!(
            w.find_contact(Side::Blue, far).is_none(),
            "3 nm is outside the buoy's effective passive range"
        );
    }

    #[test]
    fn hovering_asw_helo_lays_buoy_on_tracked_sub() {
        let mut w = world(vec![
            air(Side::Blue, "BLUE HELO", 0.0, 0.0, 30.0),
            sub(Side::Red, "RED SUB", 2.0, 0.0, 30.0),
        ]);
        let helo = id_of(&w, "BLUE HELO");
        let red = id_of(&w, "RED SUB");
        w.entity_mut(helo).unwrap().mission = Some(MissionState::Asw {
            area: Vec2::new(0.0, 0.0),
            radius_nm: 5.0,
        });

        // Seed a tracked contact above the 0.4 quality gate. The helo's
        // dipping sonar (2 nm away) refreshes it during the test, so decay
        // cannot drop it below the gate.
        seed_sub_contact(&mut w, Side::Blue, red, Vec2::new(2.0, 0.0), 0.6);

        // First eligible tick starts the 120 s cadence; no buoy before 120 s.
        ticks(&mut w, 119);
        assert_eq!(buoys(&w), 0, "no buoy before the 120 s cadence");
        ticks(&mut w, 1);
        assert_eq!(buoys(&w), 1, "one buoy after 120 s");

        let (bid, bpos, expires_at) = {
            let b = w
                .entities
                .values()
                .find(|e| e.class == SONOBUOY_CLASS)
                .expect("buoy entity");
            (b.id, b.pos, b.ext[EXT_EXPIRES_AT])
        };
        assert!((expires_at - (120.0 + SONOBUOY_LIFETIME_S as f64)).abs() < 1e-9);
        assert!(bpos.range_to(Vec2::new(2.0, 0.0)) < 0.25, "buoy at {bpos:?}");
        assert_eq!(rules_event_count(&w, "sonobuoy_laid"), 1);
        assert!(w
            .entity(bid)
            .unwrap()
            .sensors
            .iter()
            .any(|s| s.sensor == SONOBUOY_SENSOR));

        // Expiry: removing the entity and emitting the lifecycle event.
        let now = w.tick;
        w.entity_mut(bid)
            .unwrap()
            .ext
            .insert(EXT_EXPIRES_AT.to_string(), now as f64);
        ticks(&mut w, 1);
        assert_eq!(buoys(&w), 0);
        assert_eq!(rules_event_count(&w, "sonobuoy_expired"), 1);
    }

    #[test]
    fn sonobuoy_requires_hover_and_quality_above_gate() {
        let mut w = world(vec![
            air(Side::Blue, "BLUE HELO", 0.0, 0.0, 30.0),
            sub(Side::Red, "RED SUB", 2.0, 0.0, 30.0),
        ]);
        let helo = id_of(&w, "BLUE HELO");
        let red = id_of(&w, "RED SUB");
        w.entity_mut(helo).unwrap().mission = Some(MissionState::Asw {
            area: Vec2::new(0.0, 0.0),
            radius_nm: 5.0,
        });
        let cid = seed_sub_contact(&mut w, Side::Blue, red, Vec2::new(2.0, 0.0), 0.60);

        // Moving aircraft (not hovering): no buoy, timer not started.
        w.entity_mut(helo).unwrap().speed_kts = 80.0;
        w.tick = 1;
        deploy_sonobuoys(&mut w);
        w.tick = 200;
        deploy_sonobuoys(&mut w);
        assert_eq!(buoys(&w), 0, "a non-hovering ASW aircraft must not lay");

        // Hovering, but quality == 0.40 is not above the gate.
        w.entity_mut(helo).unwrap().speed_kts = 0.0;
        deploy_sonobuoys(&mut w); // starts (but does not finish) the cadence
        assert_eq!(buoys(&w), 0);
        w.entity_mut(helo)
            .unwrap()
            .ext
            .insert(EXT_BUOY_TIMER.to_string(), 0.0); // pretend 120 s elapsed
        {
            let c = w.sides[Side::Blue.index()]
                .contacts
                .get_mut(&cid)
                .unwrap();
            c.quality = 0.40;
        }
        deploy_sonobuoys(&mut w);
        assert_eq!(buoys(&w), 0, "quality must be strictly above 0.4");

        // 0.41 clears the gate.
        w.sides[Side::Blue.index()]
            .contacts
            .get_mut(&cid)
            .unwrap()
            .quality = 0.41;
        deploy_sonobuoys(&mut w);
        assert_eq!(buoys(&w), 1, "0.41 clears the quality gate");
    }

    #[test]
    fn mad_branch_detects_only_shallow_sub() {
        let mut w = world(vec![
            air(Side::Blue, "BLUE HELO", 0.0, 0.0, 30.0),
            sub(Side::Red, "SHALLOW", 0.3, 0.0, 30.0),
            sub(Side::Red, "DEEP", 0.0, 0.3, 100.0),
            sub(Side::Red, "FAR", 2.0, 0.0, 30.0),
        ]);
        let helo = id_of(&w, "BLUE HELO");

        // A 2 nm MAD set gives sensors.rs an effective 0.5 * 2 = 1 nm
        // range. MAD gating (low aircraft, shallow target) is owned by
        // sensors.rs/air.rs; the engine uses a simplified 60 m depth cutoff
        // for the official per-depth-band modifiers. This test documents
        // the branch's depth/range behavior rather than adding new
        // production code.
        w.db.sensors.insert(
            "mad_test".into(),
            SensorDef {
                radar_class: String::new(),
                id: "mad_test".into(),
                name: "Test MAD".into(),
                kind: SensorKind::Magnetic,
                air_range_nm: 0.0,
                surface_range_nm: 0.0,
                sub_range_nm: 2.0,
                missile_range_nm: 0.0,
                alt_m: 30.0,
            },
        );
        w.entity_mut(helo)
            .unwrap()
            .sensors
            .push(SensorInstance { sensor: "mad_test".into(), active: true });

        ticks(&mut w, 4);
        let shallow = id_of(&w, "SHALLOW");
        let deep = id_of(&w, "DEEP");
        let far = id_of(&w, "FAR");
        assert!(
            w.find_contact(Side::Blue, shallow).is_some(),
            "0.3 nm shallow sub should be inside the 1 nm MAD range"
        );
        assert!(
            w.find_contact(Side::Blue, deep).is_none(),
            "a 100 m submarine is below the engine's 60 m MAD cutoff"
        );
        assert!(
            w.find_contact(Side::Blue, far).is_none(),
            "a 2 nm submarine is outside the 1 nm MAD range"
        );
    }
}

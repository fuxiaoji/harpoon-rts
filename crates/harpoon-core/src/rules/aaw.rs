//! Anti-air warfare depth: three-second rule / air-burst fragments, AA gun
//! strength bands vs air, SAM director channels.
//!
//! Rules sources (`14-2.HpnV_Rules_1_Jan_26.pdf`):
//! - §8.1.3 Air Defense Gunnery, printed 8-3/8-4 (PDF pp. 83-84): AA guns
//!   fire in the point-defense band; Crossing Target Table; AA Strength ->
//!   D100 Antiair Gun Hit Chance table (printed 8-9 / PDF p. 89).
//! - §8.1.5 The Three-Second Rule, printed 8-4 (PDF p. 84): debris from a
//!   missile destroyed close to its target can strike the defender; the
//!   printed table keys the chance on missile speed and interceptor caliber
//!   (30mm-: 10/50/90%; 60mm-: 0/40/70%; 61mm+: 0/10/20%; SAM: 0/10/30%).
//!   Fragments inflict no damage points; they cause *half the airframe
//!   critical hits*, rolled on the air-burst column.
//! - §14.1.4 Airburst Damage, printed 14-3/14-4 (PDF pp. 129-130): D20
//!   "Airburst & Frag Hits" column — Weapon / Sensor / Engineering / CIC /
//!   Cargo / Flooding / Fire / Bridge / Rudder. Air-burst weapon hits never
//!   cause magazine explosions.
//! - §8.1.1 step 8 "Missile Salvoes" (printed 8-1 / PDF p. 81) and the SAM
//!   Salvo Summary (printed 6-1): each director controls one engagement of
//!   up to `salvo` missiles. The task labels the channel rule "§8.4"; in the
//!   Jan-26 book §8.4 is Antiship Missiles — the director rule is §6.1/§8.1.1.
//!
//! Adaptation notes (the task's acceptance criteria, declared here):
//! - The printed three-second table varies by missile speed and weapon
//!   caliber. The engine's hook only carries the intercept geometry, so the
//!   task's 10-90% envelope is interpolated across the 4.0 nm point-defense
//!   band (closer = deadlier). At 1.8 nm this yields ~54%, close to the
//!   printed 50% cell for a ≤30mm mount against a 751-1525 kt missile.
//! - The engine's normal impact model (`damage::apply_hit`) rolls at most one
//!   airframe critical, so "half the airframe criticals" is represented as one
//!   light airburst critical with every effect at half severity relative to
//!   `damage::roll_critical`.
//! - §14.1.4 says any armor stops fragments. `Entity` has no armor rating yet
//!   and `damage_ext::armor_factor` is a workstream stub, so the airburst roll
//!   does not consult armor; this is a known gap, not an intentional bypass.
//! - The printed AA gun rule converts AA Strength to a D100 chance; the task
//!   instead specifies a range-band table (75/55/25/15% anchors). That table
//!   is implemented here as a multiplier on the mount Pk (same structure as
//!   the surface gunnery path) and is an engine adaptation, not a literal
//!   transcription of the printed AA table. Air-specific behavior comes from
//!   the Crossing Target Table aspect multipliers.

use crate::events::{EventKind, Visibility};
use crate::rng::Rng;
use crate::types::{EntityId, PlatformKind};
use crate::world::World;

/// AAW point-defense band width in nautical miles: "Point Defense 0 - 4.0"
/// (AAW Range Bands, printed 8-4 / PDF p. 84).
pub const POINT_DEFENSE_BAND_NM: f64 = 4.0;

/// Extra DB (AA gun band data, director ratings) — filled by the AAW workstream.
pub fn db() -> crate::db::Database {
    // 训练用纯防空舰：与 ddg_aegis 同构但取消舰炮，避免在导弹防御教学关里
    // 提前用炮火击沉靶船、导致想定提前结束。仅被训练想定引用。
    const TRAINING_DDG: &str = r#"{
      "platforms": {
        "training_ddg_aa": {
          "id": "training_ddg_aa", "name": "Air-Defence Training DDG", "kind": "Ship",
          "hp": 260, "max_speed_kts": 30, "cruise_speed_kts": 20, "turn_rate_deg_s": 1.8,
          "rcs": "Medium", "noise_db": 60, "vp_value": 30,
          "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "visual"],
          "weapons": [
            {"weapon": "sam_long", "mounts": 1, "ammo": 48},
            {"weapon": "sam_point", "mounts": 1, "ammo": 24},
            {"weapon": "ciws_30mm", "mounts": 1, "ammo": 60}
          ],
          "chaff": 50, "decoys": 12
        }
      },
      "weapons": {},
      "sensors": {}
    }"#;
    const TRAINING_DDG_STRIKE: &str = r#"{
      "platforms": {
        "training_ddg_strike": {
          "id": "training_ddg_strike", "name": "Strike Training DDG", "kind": "Ship",
          "hp": 260, "max_speed_kts": 30, "cruise_speed_kts": 20, "turn_rate_deg_s": 1.8,
          "rcs": "Medium", "noise_db": 60, "vp_value": 30,
          "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "visual"],
          "weapons": [
            {"weapon": "sam_long", "mounts": 1, "ammo": 24},
            {"weapon": "ciws_30mm", "mounts": 1, "ammo": 40},
            {"weapon": "gun_5in", "mounts": 1, "ammo": 40},
            {"weapon": "ascm_std", "mounts": 2, "ammo": 4}
          ],
          "chaff": 50, "decoys": 12
        }
      },
      "weapons": {},
      "sensors": {}
    }"#;
    let mut db: crate::db::Database =
        serde_json::from_str(TRAINING_DDG).expect("training DDG definition must parse");
    let strike: crate::db::Database =
        serde_json::from_str(TRAINING_DDG_STRIKE).expect("strike DDG definition must parse");
    db = db.merged(&strike);
    db
}

// ---------------------------------------------------------------------------
// Three-second rule / air burst (§8.1.5, §14.1.4)
// ---------------------------------------------------------------------------

/// Chance that fragments from an intercept strike the defended unit.
///
/// The printed table (§8.1.5, printed 8-4 / PDF p. 84) keys on missile speed
/// and interceptor caliber. The hook does not carry either, so the task's
/// 10-90% envelope is interpolated by intercept range inside the point-defense
/// band: 90% at zero range, 10% at the 4.0 nm band edge, 0 outside the band.
/// Intercepts outside the point-defense band produce no fragment hazard.
pub fn air_burst_probability(range_nm: f64) -> f64 {
    if !(0.0..=POINT_DEFENSE_BAND_NM).contains(&range_nm) {
        return 0.0;
    }
    0.90 - 0.80 * (range_nm / POINT_DEFENSE_BAND_NM)
}

/// Called after a defensive intercept resolves. `defender` may be:
/// - the defended ship/base (CIWS path, `weapons.rs` sea-skimmer chain), or
/// - the incoming missile itself (SAM/AAM `HomingAir` path), in which case
///   the fragments threaten that missile's intended target. §8.1.5 is about
///   "the missile's intended target", not the interceptor, so we resolve the
///   defended unit from `projectile.target` and measure range from its
///   position to the intercept point.
///
/// Applies a probabilistic air-burst critical at half severity, with no
/// direct damage points (§14.1.4), and emits `RulesEvent{aaw, air_burst}`.
pub fn on_intercept(world: &mut World, defender: EntityId, range_nm: f64) {
    let Some((defender, range_nm)) = fragment_target(world, defender, range_nm) else {
        return;
    };
    let side = match world.entity(defender) {
        Some(e) if e.alive && matches!(e.kind, PlatformKind::Ship | PlatformKind::Base) => e.side,
        // AAM kills on aircraft are not ship air-burst events.
        _ => return,
    };

    let p = air_burst_probability(range_nm);
    if p <= 0.0 {
        return;
    }

    // Multiple intercepts against one ship in the same tick must draw
    // independent streams; the per-defender sequence number keys the stream.
    let seq = {
        let Some(d) = world.entity_mut(defender) else { return };
        let seq = d.ext.entry("aaw.intercept_seq".to_string()).or_insert(0.0);
        *seq += 1.0;
        *seq as u64
    };
    let mut rng = world.scope_rng(defender, &format!("aaw.air_burst.{seq}"));
    let burst = rng.chance(p);

    if let Some(d) = world.entity_mut(defender) {
        *d.ext.entry("aaw.pd_intercepts".to_string()).or_insert(0.0) += 1.0;
        d.ext.insert("aaw.last_burst_p".to_string(), p);
        if burst {
            *d.ext.entry("aaw.air_bursts".to_string()).or_insert(0.0) += 1.0;
        }
    }
    if !burst {
        return;
    }

    let (d20, effect) = roll_air_burst_critical(world, defender, &mut rng);
    let detail = format!(
        "PD-band intercept at {range_nm:.2} nmi; fragment strike chance {:.0}%; \
         airburst critical d20={d20}: {effect}",
        p * 100.0
    );
    world.emit(
        EventKind::RulesEvent {
            domain: "aaw".into(),
            rule: "air_burst".into(),
            side: Some(side),
            unit: Some(defender),
            contact: None,
            detail,
        },
        Visibility::Side(side),
    );
}

/// Resolve the unit actually threatened by the intercepted target and the
/// distance from that unit to the intercept point.
fn fragment_target(world: &World, hit: EntityId, range_nm: f64) -> Option<(EntityId, f64)> {
    let e = world.entity(hit)?;
    if e.kind == PlatformKind::Missile {
        let target = e.projectile.as_ref()?.target?;
        let t = world.entity(target)?;
        // `range_nm` at this call site is interceptor-to-missile distance
        // (~0.3 nm at merge), not the missile's distance from the ship it is
        // attacking. Measure the latter: that is the §8.1.5 geometry.
        Some((target, e.pos.range_to(t.pos)))
    } else {
        Some((hit, range_nm))
    }
}

/// One light air-burst critical. Rolls D20 on the Airburst & Frag Hits column
/// (printed 14-2 / PDF p. 128): 1-2 Weapon, 3 Sensor, 4-5 Engineering, 6 CIC,
/// 7 Sensor, 8-12 Cargo, 13-14 Flooding, 15-18 Fire, 19 Bridge, 20 Rudder.
///
/// Effects are half-severity translations onto the engine's subsystem model.
/// Air bursts never inflict direct damage points (§14.1.4) and never cause
/// magazine explosions; fire/flooding are timers consumed by
/// `damage::update_fires`, not immediate DP.
fn roll_air_burst_critical(world: &mut World, defender: EntityId, rng: &mut Rng) -> (u32, String) {
    let d20 = 1 + (rng.next_f64() * 20.0) as u32;
    let Some(d) = world.entity_mut(defender) else {
        return (d20, "no target".into());
    };
    let effect = match d20 {
        1 | 2 => {
            // Weapon: half the rounds of one armed mount (full crit empties it).
            // No magazine detonation for airbursts (§14.1.4 / PDF p. 129).
            match d
                .weapons
                .iter_mut()
                .filter(|m| m.ammo > 0)
                .max_by_key(|m| m.ammo)
            {
                Some(m) => {
                    let before = m.ammo;
                    m.ammo /= 2;
                    format!("weapon mount rounds {before} -> {} (half)", m.ammo)
                }
                None => "weapon mount (no armed mount); no effect".to_string(),
            }
        }
        3 => {
            d.systems.radar = (d.systems.radar - 0.5).max(0.0);
            format!("sensor (radar) -50% -> {:.2}", d.systems.radar)
        }
        4 | 5 => {
            // Full engineering crit removes 0.4-1.0 mobility; half severity.
            let loss = rng.range(0.2, 0.5);
            d.systems.mobility = (d.systems.mobility - loss).max(0.0);
            format!(
                "engineering: mobility -{loss:.2} -> {:.2}",
                d.systems.mobility
            )
        }
        6 => {
            let loss = rng.range(0.25, 0.5);
            d.systems.fire_control = (d.systems.fire_control - loss).max(0.0);
            format!(
                "CIC: fire control -{loss:.2} -> {:.2}",
                d.systems.fire_control
            )
        }
        7 => {
            d.systems.sonar = (d.systems.sonar - 0.5).max(0.0);
            format!("sensor (sonar) -50% -> {:.2}", d.systems.sonar)
        }
        8..=12 => "cargo/superstructure; no modeled effect".to_string(),
        13 | 14 => {
            // Half of the 120 s flooding timer used by the surface column.
            d.flooding_s = (d.flooding_s + 60.0).min(300.0);
            d.systems.mobility = (d.systems.mobility - 0.1).max(0.0);
            format!("flooding +60s -> {:.0}s", d.flooding_s)
        }
        15..=18 => {
            // Half of the 60 s fire timer used by the surface column.
            d.burn_s = (d.burn_s + 30.0).min(180.0);
            format!("fire +30s -> {:.0}s", d.burn_s)
        }
        19 => {
            d.systems.fire_control = (d.systems.fire_control - 0.5).max(0.0);
            format!(
                "bridge: fire control -0.50 -> {:.2}",
                d.systems.fire_control
            )
        }
        _ => {
            d.systems.mobility = (d.systems.mobility - 0.5).max(0.0);
            format!("rudder: mobility -0.50 -> {:.2}", d.systems.mobility)
        }
    };
    (d20, effect)
}

// ---------------------------------------------------------------------------
// AA gunnery vs air (§8.1.3)
// ---------------------------------------------------------------------------

/// AA-gun range-band base chance, as a fraction of the mount's nominal Pk.
/// Bands are 0-25%, 25-50%, 50-75%, and 75-100% of the gun's maximum range;
/// anchors are the task-specified 75/55/25/15% AA table (same band shape as
/// the GS-6 surface table, §8.3 / PDF p. 91). The printed AA rule is a
/// strength-to-D100 conversion (PDF p. 89), so this range layer is an engine
/// adaptation; the air-specific part is the aspect table below.
pub fn aa_gun_range_base(range_nm: f64, max_range_nm: f64) -> f64 {
    let frac = range_nm / max_range_nm.max(1e-9);
    if frac <= 0.25 {
        0.75
    } else if frac <= 0.5 {
        0.55
    } else if frac <= 0.75 {
        0.25
    } else {
        0.15
    }
}

/// AA aspect multiplier derived from the Crossing Target Table (§8.1.3,
/// printed 8-3 / PDF p. 83). Closing is the easiest shot, divergent next,
/// crossing hardest. The official table gives additive AA-strength modifiers
/// (-2 divergent / -4 crossing, plus speed row shifts); the engine uses the
/// declared multiplicative approximation 1.0 / 0.8 / 0.6.
///
/// `rel_deg` is the angle between the aircraft's course and the bearing from
/// the aircraft back to the shooter (0 = closing head-on).
/// `dist_nm` selects the table's range band: Point Defense ≤4 nmi,
/// Short ≤15, Medium 1 ≤30, Long 1+ beyond (PDF p. 83).
pub fn aa_gun_aspect_mult(rel_deg: f64, dist_nm: f64) -> f64 {
    let (closing_max, divergent_max) = if dist_nm <= 4.0 {
        (5.0, 15.0)
    } else if dist_nm <= 15.0 {
        (10.0, 20.0)
    } else if dist_nm <= 30.0 {
        (20.0, 45.0)
    } else {
        (60.0, 90.0)
    };
    let rel = rel_deg.abs();
    if rel <= closing_max {
        1.0
    } else if rel <= divergent_max {
        0.8
    } else {
        0.6
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::geom::Vec2;
    use crate::scenario::{load_scenario, Scenario, UnitSpawn};
    use crate::types::{ContactId, DayNight, Side};
    use crate::weapons;
    use crate::world::{Contact, Weather};

    /// Minimal armed test database. `directors` only affects `sam_test`.
    const TEST_DB_JSON: &str = r#"{
      "platforms": {
        "test_ddg": {
          "id": "test_ddg", "name": "Test DDG", "kind": "Ship",
          "hp": 200, "max_speed_kts": 30, "turn_rate_deg_s": 2.0,
          "rcs": "Medium", "vp_value": 10,
          "weapons": [
            {"weapon": "sam_test", "mounts": 1, "ammo": 20},
            {"weapon": "gun_test", "mounts": 1, "ammo": 20}
          ]
        },
        "test_air": {
          "id": "test_air", "name": "Test Aircraft", "kind": "Aircraft",
          "hp": 30, "max_speed_kts": 600, "turn_rate_deg_s": 10.0,
          "rcs": "Small", "vp_value": 10, "endurance_s": 3600
        }
      },
      "weapons": {
        "sam_test": {
          "id": "sam_test", "name": "Test SAM", "kind": "Sam",
          "targets": {"air": true, "missile": true},
          "range_nm": 40, "min_range_nm": 0.2, "speed_kts": 2000,
          "pk": 0.55, "salvo": 2, "reload_s": 0, "damage": 20,
          "guidance": "HomingAir", "directors": __DIRECTORS__
        },
        "gun_test": {
          "id": "gun_test", "name": "Test AA Gun", "kind": "Gun",
          "targets": {"air": true, "surface": true},
          "range_nm": 4, "speed_kts": 0, "pk": 1.0, "salvo": 3,
          "reload_s": 0, "damage": 8, "guidance": "BallisticGun"
        },
        "ascm_test": {
          "id": "ascm_test", "name": "Test ASM", "kind": "Ascm",
          "targets": {"surface": true},
          "range_nm": 60, "min_range_nm": 3, "speed_kts": 560,
          "pk": 0.55, "salvo": 2, "reload_s": 30, "damage": 40,
          "guidance": "SeaSkimmer"
        }
      },
      "sensors": {}
    }"#;

    fn test_db(directors: u8) -> Database {
        let json = TEST_DB_JSON.replace("__DIRECTORS__", &directors.to_string());
        Database::from_json(&json).expect("test db")
    }

    /// Blue DDG at origin + Red aircraft at `target_pos`; a perfect Blue
    /// contact is inserted so free fire control can engage immediately.
    fn test_world(directors: u8, target_pos: Vec2, target_course: f64) -> (World, EntityId, EntityId) {
        let sc = Scenario {
            id: "aaw-test".into(),
            name: "AAW test".into(),
            description: String::new(),
            context: Default::default(),
            duration_s: 600,
            weather: Weather {
                visibility_nm: 20.0,
                jam: [0.0, 0.0],
                overcast: 0.0,
                rain: 0.0,
            },
            day_night: DayNight::Day,
            seed: 20261018,
            victory: Default::default(),
            units: vec![
                UnitSpawn {
                    side: Side::Blue,
                    class: "test_ddg".into(),
                    name: Some("BLUE DDG".into()),
                    pos: Vec2::zero(),
                    course_deg: 0.0,
                    speed_kts: 10.0,
                    depth_m: None,
                    altitude_m: None,
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: true,
                },
                UnitSpawn {
                    side: Side::Red,
                    class: "test_air".into(),
                    name: Some("RED AIR".into()),
                    pos: target_pos,
                    course_deg: target_course,
                    speed_kts: 300.0,
                    depth_m: None,
                    altitude_m: Some(1000.0),
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: false,
                },
            ],
        };
        let mut w = load_scenario(sc, test_db(directors)).expect("test scenario loads");
        let blue = w.entities.values().find(|e| e.side == Side::Blue).unwrap().id;
        let red = w.entities.values().find(|e| e.side == Side::Red).unwrap().id;
        let pos = w.entity(red).unwrap().pos;
        w.sides[Side::Blue.index()].contacts.insert(
            ContactId(1),
            Contact {
                id: ContactId(1),
                about: red,
                category: PlatformKind::Aircraft,
                class_name: None,
                estimate: pos,
                est_course_deg: target_course,
                est_speed_kts: 300.0,
                bearing_only: false,
                quality: 1.0,
                last_update_tick: 0,
                first_tick: 0,
                stale: false,
                sensors: Vec::new(),
                offset: Vec2::zero(),
                announced: true,
            },
        );
        (w, blue, red)
    }

    fn air_burst_events(w: &World) -> Vec<&crate::events::Event> {
        w.events
            .iter()
            .filter(|e| {
                matches!(
                    &e.kind,
                    EventKind::RulesEvent { domain, rule, .. }
                        if domain == "aaw" && rule == "air_burst"
                )
            })
            .collect()
    }

    fn gun_hits(w: &World) -> Option<u8> {
        w.events.iter().rev().find_map(|e| match &e.kind {
            EventKind::GunAction { hits, .. } => Some(*hits),
            _ => None,
        })
    }

    fn in_flight(w: &World) -> usize {
        w.entities
            .values()
            .filter(|e| e.alive && e.kind == PlatformKind::Missile)
            .count()
    }

    #[test]
    fn air_burst_probability_interpolates_across_pd_band() {
        assert!((air_burst_probability(0.0) - 0.90).abs() < 1e-9);
        assert!((air_burst_probability(2.0) - 0.50).abs() < 1e-9);
        assert!((air_burst_probability(4.0) - 0.10).abs() < 1e-9);
        assert_eq!(air_burst_probability(4.01), 0.0);
        assert_eq!(air_burst_probability(30.0), 0.0);
    }

    #[test]
    fn air_burst_only_inside_point_defense_band() {
        let (mut w, blue, _) = test_world(0, Vec2::new(10.0, 0.0), 270.0);
        // Outside the 4 nm PD band: no fragment hazard (e.g. long-range SAM).
        for _ in 0..40 {
            on_intercept(&mut w, blue, 6.0);
        }
        assert_eq!(air_burst_events(&w).len(), 0);
        assert_eq!(
            w.entity(blue).unwrap().ext.get("aaw.air_bursts").copied().unwrap_or(0.0),
            0.0
        );

        // 0.5 nm -> p = 0.80; 40 independent draws make a hit all but certain.
        for _ in 0..40 {
            on_intercept(&mut w, blue, 0.5);
        }
        let events = air_burst_events(&w);
        assert!(!events.is_empty(), "close intercepts must be able to air-burst");
        for e in &events {
            match &e.kind {
                EventKind::RulesEvent {
                    side,
                    unit,
                    detail,
                    ..
                } => {
                    assert_eq!(*side, Some(Side::Blue));
                    assert_eq!(*unit, Some(blue));
                    assert!(detail.contains("airburst critical"), "detail: {detail}");
                }
                _ => unreachable!(),
            }
        }
        // Only in-band intercepts increment the counter (out-of-band calls
        // return before the sequence bookkeeping).
        assert!(w.entity(blue).unwrap().ext["aaw.pd_intercepts"] >= 40.0);
    }

    #[test]
    fn air_burst_is_light_and_never_direct_dp() {
        let (mut w, blue, _) = test_world(0, Vec2::new(10.0, 0.0), 270.0);
        let hp_before = w.entity(blue).unwrap().hp;
        for _ in 0..100 {
            on_intercept(&mut w, blue, 0.5);
        }
        assert!(!air_burst_events(&w).is_empty());
        let d = w.entity(blue).unwrap();
        // §14.1.4: airbursts inflict no damage points.
        assert!((d.hp - hp_before).abs() < 1e-9, "air burst must not cost DP");
        // A half-severity subsystem effect, fire/flooding timer, or half ammo
        // loss must have landed across ~80 bursts (cargo is only 5/20 rows).
        let systems_changed = d.systems.radar < 1.0
            || d.systems.sonar < 1.0
            || d.systems.mobility < 1.0
            || d.systems.fire_control < 1.0;
        let timers_changed = d.burn_s > 0.0 || d.flooding_s > 0.0;
        let ammo_changed = d
            .weapons
            .iter()
            .any(|m| m.weapon == "sam_test" && m.ammo < 20);
        assert!(
            systems_changed || timers_changed || ammo_changed,
            "air-burst crits never changed any light-damage state"
        );
    }

    #[test]
    fn air_burst_against_incoming_missile_credits_the_ship() {
        let (mut w, blue, red) = test_world(0, Vec2::new(10.0, 0.0), 270.0);
        let ascm = w.db.weapons.get("ascm_test").unwrap().clone();
        let mid = weapons::launch(&mut w, red, &ascm, Some(blue), None).expect("missile launch");
        // Intercept point 0.5 nmi from the ship it is attacking.
        w.entity_mut(mid).unwrap().pos = Vec2::new(0.5, 0.0);
        for _ in 0..30 {
            // The HomingAir call site passes interceptor-to-target range
            // (~0.3 nm); on_intercept must use the missile-to-ship geometry.
            on_intercept(&mut w, mid, 0.3);
        }
        let events = air_burst_events(&w);
        assert!(!events.is_empty());
        for e in &events {
            match &e.kind {
                EventKind::RulesEvent { unit, detail, .. } => {
                    assert_eq!(*unit, Some(blue), "fragments must threaten the defended ship");
                    assert!(detail.contains("0.50 nmi"), "detail: {detail}");
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn sam_director_channels_cap_in_flight_salvos() {
        // directors = 1, salvo = 2 -> at most 2 live missiles per weapon type.
        let (mut w, _, _) = test_world(1, Vec2::new(10.0, 0.0), 270.0);
        weapons::fire_control_scan(&mut w);
        assert_eq!(in_flight(&w), 2, "first engagement launches one salvo");
        weapons::fire_control_scan(&mut w);
        assert_eq!(in_flight(&w), 2, "channels full: no second engagement");

        // Once the salvo is gone, the director can engage again.
        w.entities.retain(|_, e| !e.kind.is_projectile());
        weapons::fire_control_scan(&mut w);
        assert_eq!(in_flight(&w), 2, "channels free again after intercept/runout");

        // directors = 2 -> two engagements (4 missiles) may be in flight.
        let (mut w2, _, _) = test_world(2, Vec2::new(10.0, 0.0), 270.0);
        weapons::fire_control_scan(&mut w2);
        assert_eq!(in_flight(&w2), 2);
        weapons::fire_control_scan(&mut w2);
        assert_eq!(in_flight(&w2), 4);
        weapons::fire_control_scan(&mut w2);
        assert_eq!(in_flight(&w2), 4, "two director channels are now occupied");
    }

    #[test]
    fn sam_without_director_limit_can_stack_salvos() {
        // directors = 0 is the DB default: unlimited channels.
        let (mut w, _, _) = test_world(0, Vec2::new(10.0, 0.0), 270.0);
        weapons::fire_control_scan(&mut w);
        weapons::fire_control_scan(&mut w);
        assert_eq!(in_flight(&w), 4, "0 = unlimited (two salvos of two)");
    }

    #[test]
    fn aa_gun_band_and_aspect_tables() {
        assert!((aa_gun_range_base(1.0, 4.0) - 0.75).abs() < 1e-9); // 0-25%
        assert!((aa_gun_range_base(2.0, 4.0) - 0.55).abs() < 1e-9); // 25-50%
        assert!((aa_gun_range_base(3.0, 4.0) - 0.25).abs() < 1e-9); // 50-75%
        assert!((aa_gun_range_base(3.9, 4.0) - 0.15).abs() < 1e-9); // 75-100%

        // Crossing Target Table buckets (PDF p. 83).
        assert_eq!(aa_gun_aspect_mult(0.0, 1.0), 1.0); // closing
        assert_eq!(aa_gun_aspect_mult(10.0, 1.0), 0.8); // divergent
        assert_eq!(aa_gun_aspect_mult(30.0, 1.0), 0.6); // crossing
        assert_eq!(aa_gun_aspect_mult(40.0, 20.0), 0.8); // Medium 1 divergent
        assert_eq!(aa_gun_aspect_mult(80.0, 20.0), 0.6); // Medium 1 crossing
    }

    #[test]
    fn aa_gun_engages_aircraft_on_air_band_path() {
        // Gun boat: remove the SAM mount so the Gun branch is exercised.
        let (mut w, blue, _) = test_world(0, Vec2::new(1.0, 0.0), 270.0);
        w.entity_mut(blue)
            .unwrap()
            .weapons
            .retain(|m| m.weapon == "gun_test");
        weapons::fire_control_scan(&mut w);
        let close_hits = gun_hits(&w).expect("AA gun must fire at an aircraft");
        assert!(close_hits > 0, "0-25% band at 1.0/4.0 nm should hit");
        assert!(close_hits <= 3);

        // Same seed, 3.8 nm = 75-100% band (0.15 base): far fewer hits.
        let (mut w2, blue2, _) = test_world(0, Vec2::new(3.8, 0.0), 270.0);
        w2.entity_mut(blue2)
            .unwrap()
            .weapons
            .retain(|m| m.weapon == "gun_test");
        weapons::fire_control_scan(&mut w2);
        let far_hits = gun_hits(&w2).expect("AA gun must fire at long band too");
        assert!(
            close_hits > far_hits,
            "air range bands must favour the near band: close={close_hits} far={far_hits}"
        );
    }

    #[test]
    fn non_aa_gun_cannot_engage_aircraft() {
        let (mut w, blue, _) = test_world(0, Vec2::new(1.0, 0.0), 270.0);
        {
            let gun = w.db.weapons.get_mut("gun_test").unwrap();
            gun.targets.air = false;
        }
        w.entity_mut(blue)
            .unwrap()
            .weapons
            .retain(|m| m.weapon == "gun_test");
        weapons::fire_control_scan(&mut w);
        assert!(gun_hits(&w).is_none(), "`targets.air: false` must gate air fire");
    }
}

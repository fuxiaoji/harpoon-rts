//! Sensing rules owned by the sensing workstream:
//! radar generation / clutter (§5.2.4, §5.2.5), EO/IR weather attenuation
//! (§5.5) and a first-order airborne-ECM rule (§12.1/§12.5).
//!
//! Rules sources (`14-2.HpnV_Rules_1_Jan_26.pdf`):
//! - §5.2.4 Shipboard Radar Types, printed 5-2 (PDF p. 41): air-search
//!   radars have trouble with VLow/SSkim contacts because of sea clutter;
//!   surface-search radars are optimised for surface/low targets.
//! - §5.2.9.1/5.2.9.2 and the Clutter Effects on Range table, printed
//!   5-4/5-5 (PDF pp. 43-44): net clutter value 3 gives a ×0.60 range
//!   modifier. The engine uses one fixed cell (sea state 3) as the LAS
//!   low-target multiplier.
//! - §5.2.5 Airborne Radar Types, printed 5-3 (PDF p. 42): airborne SS
//!   radars pick up VLow/SSkim contacts at one-tenth range, and "other than
//!   the effects of line of sight, their detection capability is not
//!   affected by altitude". The SS up-look penalty below is therefore an
//!   engine abstraction, not a literal rule.
//! - Altitude Bands table, printed 4-3 (PDF p. 36): the High band starts at
//!   7501 m (Low 61-2000 m, Medium 2001-7500 m, High 7501-15500 m).
//! - §5.5 Electro-Optical/Infrared Sensors, printed 5-20 (PDF p. 59):
//!   "IR ranges are reduced by water in the air"; the precipitation table
//!   gives ×0.90 (drizzle/light fog), ×0.75 (light rain/moderate fog),
//!   ×0.50 (moderate rain/heavy fog), ×0.25 (heavy rain).
//! - §5.8.7.1 Clouds, printed 5-27 (PDF p. 66): solid overcast blocks
//!   visual line of sight; cloud cover is expressed as 0/25/50/75/100%.
//! - §12.1 Airborne ECM, printed 12-1 (PDF p. 119) and §12.5 Noise
//!   Jamming: noise jammers reduce search-radar detection range; the real
//!   effect depends on the jammer's clutter rating and facet geometry
//!   (Annex B/H5), which the engine does not model yet.
//!
//! Work-order adaptations (values marked estimated where the tabletop rule
//! is a table or a geometry-dependent procedure):
//! - LAS radar vs an airborne target below 50 m: ×0.60.  The 50 m cut is
//!   the work order's simplification of the 0-20 m SSkim and 21-60 m VLow
//!   bands; ×0.60 is the printed net-clutter-3 cell (estimated choice).
//! - SS radar vs an aircraft at or above 7501 m: ×0.30 (estimated
//!   up-look blind-zone abstraction).
//! - SS radar when rain > 0.5: ×0.85 (estimated; the printed heavy-rain
//!   clutter row would be ×0.25 and applies to all radar types).
//! - LAS radar vs a target whose class id contains "jam": ×0.50
//!   (estimated simplification of §12.5; no range/sector/geometry test).
//! - Overcast: visual/IR ×(1 - 0.25 × overcast); rain: ×(1 - 0.3 × rain).
//!   Both are continuous engine abstractions of the printed discrete
//!   weather rows.
//!
//! Wiring: `sensors.rs` calls the two hooks with only `(def, target)` /
//! `(observer, target)`, so weather rides on `Entity.ext` (keys
//! [`EXT_OVERCAST`] / [`EXT_RAIN`]) and is refreshed by [`step`], which
//! `rules::step` invokes once per tick before `sensors::update_sensors`.
//! Entities created later in the same tick (e.g. a missile launched by
//! `weapons::update_weapons`) pick up the conditions on the next tick.

use crate::db::{Database, PlatformClass, SensorDef, SensorKind};
use crate::types::{PlatformKind, RcsClass};
use crate::world::{Entity, World};

// ---------------------------------------------------------------------------
// Tuning constants (see module header for provenance)
// ---------------------------------------------------------------------------

/// §5.2.4/§5.2.9 clutter cell: LAS radar vs low/sea-skimming air target.
pub const LAS_LOW_TARGET_MULT: f64 = 0.60;
/// Estimated SS up-look blind-zone multiplier vs High-band aircraft.
pub const SS_HIGH_AIRCRAFT_MULT: f64 = 0.30;
/// Estimated additional SS rain-clutter multiplier when rain > 0.5.
pub const SS_RAIN_MULT: f64 = 0.85;
/// Estimated first-order noise-jamming multiplier for an LAS radar.
pub const JAMMER_TARGET_MULT: f64 = 0.50;
/// Work-order low-altitude cut for LAS clutter (SSkim/VLow boundary).
pub const LOW_AIR_TARGET_M: f64 = 50.0;
/// Floor of the High altitude band (Altitude Bands table, printed 4-3).
pub const HIGH_AIRCRAFT_M: f64 = 7501.0;
/// Rain intensity above which the SS rain term applies (work order).
pub const SS_RAIN_THRESHOLD: f64 = 0.5;
/// Visual/IR attenuation per unit of overcast fraction (§5.8.7.1 adaptation).
pub const OVERCAST_VISUAL_PER_FRACTION: f64 = 0.25;
/// Visual/IR attenuation per unit of rain fraction (§5.5 adaptation).
pub const RAIN_VISUAL_PER_FRACTION: f64 = 0.30;

/// `Entity.ext` key carrying the scenario's overcast fraction (0..1).
pub const EXT_OVERCAST: &str = "sensing.overcast";
/// `Entity.ext` key carrying the scenario's rain fraction (0..1).
pub const EXT_RAIN: &str = "sensing.rain";

/// Aircraft platform class carrying a noise jammer (§12.1/§12.5).
pub const JAMMER_CLASS: &str = "fighter_jam";
/// Marker sensor for the jammer pod; the jamming effect itself is the
/// static class rule in [`radar_type_modifier`].
pub const JAMMER_SENSOR: &str = "jammer_x";

// ---------------------------------------------------------------------------
// Database extension
// ---------------------------------------------------------------------------

/// Adds the ECM aircraft used by the simplified §12.1 rule.
///
/// Values are generic late-Cold-War/1990s stand-in figures (estimated):
/// a Small-RCS two-seat EW aircraft with a large passive ESM fit and a high
/// acoustic source level.  It carries no weapons and no deck.
pub fn db() -> Database {
    let mut db = Database::default();
    db.sensors.insert(
        JAMMER_SENSOR.to_string(),
        SensorDef {
            id: JAMMER_SENSOR.to_string(),
            name: "X-band Noise Jammer".to_string(),
            // The pod is an emitter, not a receiver.  It is modelled as an
            // Esm entry with zero ranges so it never creates a detection by
            // itself; enemies still detect the aircraft through the class's
            // radar/ESM signature rules.
            kind: SensorKind::Esm,
            air_range_nm: 0.0,
            surface_range_nm: 0.0,
            sub_range_nm: 0.0,
            missile_range_nm: 0.0,
            alt_m: 8000.0,
            radar_class: String::new(),
        },
    );
    db.platforms.insert(
        JAMMER_CLASS.to_string(),
        PlatformClass {
            id: JAMMER_CLASS.to_string(),
            name: "Fighter-Jammer (ECM)".to_string(),
            kind: PlatformKind::Aircraft,
            hp: 30.0,
            max_speed_kts: 1000.0,
            cruise_speed_kts: 420.0,
            turn_rate_deg_s: 9.0,
            accel_kts_per_s: 0.8,
            rcs: RcsClass::Small,
            // Loud relative to a fighter (default 50 dB).
            noise_db: 85.0,
            visual_factor: 1.0,
            vp_value: 18.0,
            sensors: vec!["esm_std".to_string(), JAMMER_SENSOR.to_string()],
            weapons: Vec::new(),
            chaff: 0,
            decoys: 0,
            endurance_s: 14_400.0,
            max_depth_m: 0.0,
            dipping_sonar: false,
            is_base: false,
            deck: None,
        },
    );
    db
}

// ---------------------------------------------------------------------------
// Weather publication
// ---------------------------------------------------------------------------

/// Copy the scenario weather into every entity's `ext`, where the stateless
/// sensor hooks can reach it.  Intended to run once per tick before
/// `sensors::update_sensors` (see the wiring note in the module header).
pub fn step(world: &mut World) {
    let overcast = world.weather.overcast.clamp(0.0, 1.0);
    let rain = world.weather.rain.clamp(0.0, 1.0);
    for e in world.entities.values_mut() {
        if e.ext.get(EXT_OVERCAST).copied() != Some(overcast) {
            e.ext.insert(EXT_OVERCAST.to_string(), overcast);
        }
        if e.ext.get(EXT_RAIN).copied() != Some(rain) {
            e.ext.insert(EXT_RAIN.to_string(), rain);
        }
    }
}

/// Read a 0..1 condition from the observer, falling back to the target (the
/// `radar_type_modifier` hook has no observer parameter).  Missing data means
/// clear weather, keeping the hook neutral when it is called outside the tick
/// loop.
fn condition(primary: &Entity, fallback: &Entity, key: &str) -> f64 {
    primary
        .ext
        .get(key)
        .or_else(|| fallback.ext.get(key))
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Radar generation / weather (§5.2.4, §5.2.5, §5.2.9, §12.5)
// ---------------------------------------------------------------------------

fn is_low_air_target(t: &Entity) -> bool {
    matches!(t.kind, PlatformKind::Aircraft | PlatformKind::Missile)
        && t.altitude_m < LOW_AIR_TARGET_M
}

fn is_jammer_class(t: &Entity) -> bool {
    t.class.to_ascii_lowercase().contains("jam")
}

/// Radar-generation modifier applied on top of the effective-range chain.
///
/// `def.radar_class` is `"LAS"` (long-range air search) or `"SS"` (surface
/// search); any other/empty value is neutral.  The hook is called only for
/// active radar sensors by `sensors.rs`.
pub fn radar_type_modifier(def: &SensorDef, t: &Entity) -> f64 {
    let mut mult = 1.0;
    let radar_class = def.radar_class.trim();

    if radar_class.eq_ignore_ascii_case("LAS") {
        // Air-search sets lose low/sea-skimming targets in sea clutter
        // (§5.2.4/§5.2.9.1).
        if is_low_air_target(t) {
            mult *= LAS_LOW_TARGET_MULT;
        }
        // Simplified noise jamming: a jamming target degrades the search
        // radar's range (§12.5).  Range/sector geometry is not modelled.
        if is_jammer_class(t) {
            mult *= JAMMER_TARGET_MULT;
        }
    } else if radar_class.eq_ignore_ascii_case("SS") {
        // Estimated up-look blind zone: surface-search sets are optimised for
        // the surface/low bands, not High-band aircraft (§5.2.4).
        if t.kind == PlatformKind::Aircraft && t.altitude_m >= HIGH_AIRCRAFT_M {
            mult *= SS_HIGH_AIRCRAFT_MULT;
        }
        // Rain clutter, work-order threshold and multiplier.
        if condition(t, t, EXT_RAIN) > SS_RAIN_THRESHOLD {
            mult *= SS_RAIN_MULT;
        }
    }

    mult
}

// ---------------------------------------------------------------------------
// EO/IR and visual weather (§5.5, §5.8.7.1)
// ---------------------------------------------------------------------------

/// Visual / EO-IR weather modifier.  Day/night is handled by the existing
/// `DayNight` path in `sensors.rs` and is deliberately not repeated here.
pub fn visual_modifier(o: &Entity, t: &Entity) -> f64 {
    let overcast = condition(o, t, EXT_OVERCAST);
    let rain = condition(o, t, EXT_RAIN);
    let mult = (1.0 - OVERCAST_VISUAL_PER_FRACTION * overcast)
        * (1.0 - RAIN_VISUAL_PER_FRACTION * rain);
    mult.clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DayNight, Side};
    use crate::world::Weather;
    use crate::{Database, Scenario, UnitSpawn, Vec2, load_scenario};

    fn scenario(units: Vec<UnitSpawn>, overcast: f64, rain: f64) -> Scenario {
        Scenario {
            id: "sensing-test".into(),
            name: "Sensing test".into(),
            description: String::new(),
            context: Default::default(),
            duration_s: 600,
            weather: Weather {
                visibility_nm: 12.0,
                jam: [0.0, 0.0],
                overcast,
                rain,
            },
            day_night: DayNight::Day,
            seed: 11,
            victory: Default::default(),
            units,
        }
    }

    fn unit(side: Side, class: &str, name: &str, altitude_m: Option<f64>) -> UnitSpawn {
        UnitSpawn {
            side,
            class: class.into(),
            name: Some(name.into()),
            pos: Vec2::new(10.0, 0.0),
            course_deg: 0.0,
            speed_kts: 300.0,
            depth_m: None,
            altitude_m,
            waypoints: None,
            follow: None,
            sonar_active: false,
            fire_on: false,
        }
    }

    fn world(class: &str, altitude_m: f64) -> World {
        load_scenario(
            scenario(
                vec![unit(Side::Red, class, "TGT", Some(altitude_m))],
                0.0,
                0.0,
            ),
            Database::built_in(),
        )
        .expect("test scenario must load")
    }

    fn target(class: &str, altitude_m: f64) -> Entity {
        world(class, altitude_m)
            .entities
            .values()
            .next()
            .cloned()
            .expect("test world has one entity")
    }

    fn radar(radar_class: &str) -> SensorDef {
        SensorDef {
            id: format!("radar_{radar_class}"),
            name: format!("{radar_class} radar"),
            kind: SensorKind::Radar,
            air_range_nm: 100.0,
            surface_range_nm: 40.0,
            sub_range_nm: 0.0,
            missile_range_nm: 50.0,
            alt_m: 25.0,
            radar_class: radar_class.into(),
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn las_air_search_loses_sea_skimming_targets() {
        let las = radar("LAS");

        let mut aircraft = target("fighter", 49.0);
        assert!(close(radar_type_modifier(&las, &aircraft), 0.60));

        aircraft.altitude_m = 50.0;
        assert!(
            close(radar_type_modifier(&las, &aircraft), 1.0),
            "50 m is the work order's boundary; only < 50 m is penalised"
        );

        let mut missile = target("fighter", 8.0);
        missile.kind = PlatformKind::Missile;
        assert!(
            close(radar_type_modifier(&las, &missile), 0.60),
            "sea-skimmers spawn at 8 m (weapons.rs), well inside the cut"
        );

        let mut ship = target("fighter", 0.0);
        ship.kind = PlatformKind::Ship;
        assert!(
            close(radar_type_modifier(&las, &ship), 1.0),
            "zero-altitude surface contacts are not low-flying air targets"
        );
    }

    #[test]
    fn ss_surface_search_has_high_aircraft_blind_zone() {
        let ss = radar("SS");

        let mut aircraft = target("fighter", 7501.0);
        assert!(close(radar_type_modifier(&ss, &aircraft), 0.30));

        aircraft.altitude_m = 7500.0;
        assert!(
            close(radar_type_modifier(&ss, &aircraft), 1.0),
            "7500 m is still the Medium band (7501 m starts High)"
        );

        let mut missile = target("fighter", 10_000.0);
        missile.kind = PlatformKind::Missile;
        assert!(
            close(radar_type_modifier(&ss, &missile), 1.0),
            "the work order applies the up-look penalty to aircraft only"
        );

        let las = radar("LAS");
        let high_aircraft = target("fighter", 9000.0);
        assert!(close(radar_type_modifier(&las, &high_aircraft), 1.0));
    }

    #[test]
    fn rain_degrades_ss_radar_above_half_intensity() {
        let ss = radar("SS");
        let las = radar("LAS");
        let mut aircraft = target("fighter", 1000.0);

        aircraft.ext.insert(EXT_RAIN.to_string(), 0.6);
        assert!(
            close(radar_type_modifier(&ss, &aircraft), 0.85),
            "rain 0.6 must apply the ×0.85 SS rain term"
        );
        assert!(
            close(radar_type_modifier(&las, &aircraft), 1.0),
            "the rain term is SS-only in this work order"
        );

        aircraft.ext.insert(EXT_RAIN.to_string(), 0.5);
        assert!(
            close(radar_type_modifier(&ss, &aircraft), 1.0),
            "the threshold is strictly greater than 0.5"
        );

        // Stacking: high aircraft in rain folds both SS penalties.
        let high = target("fighter", 8000.0);
        let mut high_rain = high;
        high_rain.ext.insert(EXT_RAIN.to_string(), 0.8);
        assert!(close(radar_type_modifier(&ss, &high_rain), 0.30 * 0.85));
    }

    #[test]
    fn jammer_class_halves_las_radar_range() {
        let las = radar("LAS");
        let ss = radar("SS");

        let jammer = target(JAMMER_CLASS, 6000.0);
        assert!(close(radar_type_modifier(&las, &jammer), 0.50));
        assert!(
            close(radar_type_modifier(&ss, &jammer), 1.0),
            "the first-order jammer rule is LAS-only"
        );

        let fighter = target("fighter", 6000.0);
        assert!(close(radar_type_modifier(&las, &fighter), 1.0));
    }

    #[test]
    fn visual_modifier_combines_overcast_and_rain() {
        let mut o = target("fighter", 6000.0);
        let t = o.clone();
        assert!(
            close(visual_modifier(&o, &t), 1.0),
            "no weather in ext means clear conditions"
        );

        o.ext.insert(EXT_OVERCAST.to_string(), 0.5);
        o.ext.insert(EXT_RAIN.to_string(), 0.5);
        assert!(close(visual_modifier(&o, &t), 0.875 * 0.85));

        o.ext.insert(EXT_OVERCAST.to_string(), 1.0);
        o.ext.insert(EXT_RAIN.to_string(), 1.0);
        assert!(close(visual_modifier(&o, &t), 0.75 * 0.70));

        // The target's ext is a fallback when the observer has no weather.
        let mut t2 = target("fighter", 6000.0);
        t2.ext.insert(EXT_OVERCAST.to_string(), 0.4);
        t2.ext.insert(EXT_RAIN.to_string(), 0.2);
        let clear_observer = target("fighter", 6000.0);
        assert!(close(
            visual_modifier(&clear_observer, &t2),
            0.9 * 0.94
        ));
    }

    #[test]
    fn step_publishes_clamped_weather_to_every_entity() {
        let mut w = load_scenario(
            scenario(vec![unit(Side::Blue, "fighter", "BLUE", Some(6000.0))], 1.4, -0.2),
            Database::built_in(),
        )
        .expect("test scenario must load");

        step(&mut w);

        for e in w.entities.values() {
            assert!(close(e.ext[EXT_OVERCAST], 1.0));
            assert!(close(e.ext[EXT_RAIN], 0.0));
        }

        // The stateless hook now sees the published conditions.
        let mut w = load_scenario(
            scenario(vec![unit(Side::Blue, "fighter", "BLUE", Some(6000.0))], 0.8, 0.4),
            Database::built_in(),
        )
        .expect("test scenario must load");
        step(&mut w);
        let e = w.entities.values().next().unwrap();
        assert!(close(visual_modifier(e, e), 0.80 * 0.88));
    }

    #[test]
    fn rules_step_wires_sensing_weather_into_the_tick_pass() {
        let mut w = load_scenario(
            scenario(vec![unit(Side::Blue, "fighter", "BLUE", Some(6000.0))], 0.3, 0.7),
            Database::built_in(),
        )
        .expect("test scenario must load");

        crate::rules::step(&mut w);

        let e = w.entities.values().next().unwrap();
        assert!(close(e.ext[EXT_OVERCAST], 0.3));
        assert!(close(e.ext[EXT_RAIN], 0.7));
    }

    #[test]
    fn db_defines_fighter_jammer_platform_and_marker_sensor() {
        // This layer references `esm_std`, which comes from the canonical
        // built-in database, so validate the merged database rather than the
        // standalone extension.
        let db = db();
        let merged = Database::built_in().merged(&db);
        merged
            .validate()
            .expect("merged sensing db must be internally consistent");

        let p = merged
            .platforms
            .get(JAMMER_CLASS)
            .expect("fighter_jam platform must exist");
        assert_eq!(p.kind, PlatformKind::Aircraft);
        assert_eq!(p.rcs, RcsClass::Small);
        assert!(p.sensors.contains(&"esm_std".to_string()));
        assert!(p.sensors.contains(&JAMMER_SENSOR.to_string()));

        let sensor = db
            .sensors
            .get(JAMMER_SENSOR)
            .expect("jammer_x sensor must exist");
        assert_eq!(sensor.kind, SensorKind::Esm);
        assert_eq!(sensor.air_range_nm, 0.0);

        // The registry merge in Database::built_in must include it.
        let built_in = Database::built_in();
        assert!(built_in.platforms.contains_key(JAMMER_CLASS));
        assert!(built_in.sensors.contains_key(JAMMER_SENSOR));
    }
}

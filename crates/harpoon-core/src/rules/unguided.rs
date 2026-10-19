//! §9.6 unguided air attack: iron bombs, rockets, and delivery geometry.
//!
//! Rules basis (`14-2.HpnV_Rules_1_Jan_26.pdf`):
//! - §9.6.1 (PDF p.105 / printed 9-4): bombsight quality drives unguided
//!   accuracy; advanced sights give near-guided accuracy from low altitude.
//! - §9.6.4 (PDF pp.105-106 / printed 9-4..9-5): level, glide, and lob-toss
//!   bombing. Low/VLow level bombing must overfly the target; glide bombing
//!   releases from Low or Medium.
//! - §9.6.5 (PDF pp.106-107 / printed 9-5..9-6): unguided bomb attack table.
//!   Level-bombing altitude modifiers run from ×3.0 below 1,500 m to ×0.25
//!   at 5,000-7,500 m; glide bombing released at Medium altitude is "down 2
//!   rows".
//! - §9.6.7 (PDF p.108 / printed 9-7): rockets require a direct overflight
//!   at Low/VLow, use the glide-bombing hit chance, and are fired in pairs.
//! - §7.3 (PDF p.76 / printed 7-1): weapons danger space for stray gunfire
//!   and missiles; §9.6 (PDF p.105) says a weapon that misses a ground
//!   target has no other effect.
//!
//! Engine integration:
//! - [`db`] adds `bomb_mk82`, `rocket_57`, and `strike_attacker`.
//!   `weapons.rs::fire_control_scan` already routes Bomb/Rocket through
//!   `gun_salvo` (instant adjudication, no projectile) behind a `< 900 m`
//!   altitude gate, so `def.pk` is the live single-round hit chance.
//!   The `pk` values below are calibrated for the nominal low-altitude
//!   overhead delivery; [`delivery_modifier`] documents the altitude/speed
//!   bands but the current pipeline does not multiply it in. This is the
//!   declared simplification "altitude-band correction absorbed into the
//!   calibrated db pk".
//! - [`danger_space_nm`] is a scenario-author geometry helper. Live
//!   friendly-fire adjudication is NOT wired into [`step`]; see that
//!   function's docs for the rule basis and limits.
//!
//! [`step`] is intentionally a no-op: bombs and rockets resolve instantly
//! and no cross-fire state advances per tick.

use crate::db::Database;
use crate::world::World;

/// Below this release altitude the unguided delivery gets its low-altitude
/// bonus. The 300 m / 600 m breakpoints and 1.2 / 0.7 / 0.8 factors are an
/// RTS calibration of §9.6.5's altitude-row shifts, **not** published
/// tabletop values (estimated; see [`delivery_modifier`]).
const LOW_ALT_BAND_M: f64 = 300.0;
/// Above this release altitude the delivery penalty applies.
const HIGH_ALT_BAND_M: f64 = 600.0;
const LOW_ALT_MODIFIER: f64 = 1.2;
const HIGH_ALT_MODIFIER: f64 = 0.7;
/// Fast-release penalty threshold, estimated against the delivery-profile
/// discussion in §9.6.4/§9.6.5 (no tabletop speed band for bombs).
const FAST_RELEASE_KTS: f64 = 450.0;
const FAST_RELEASE_MODIFIER: f64 = 0.8;

/// Extra DB entries for §9.6 unguided air ordnance.
///
/// Work-order values (RTS adaptation):
/// - `bomb_mk82`: range 0.8 nmi (overhead release, §9.6.4), damage 30,
///   single-bomb Pk 0.35, one bomb per release.
/// - `rocket_57`: range 3 nmi, damage 12 and Pk 0.25 per rocket, salvo 4 =
///   two pairs (§9.6.7).
/// - `strike_attacker`: 500 kt max speed, Small RCS, 7,200 s endurance,
///   four bombs and one four-rocket pod.
///
/// Estimated values (no tabletop source; RTS calibration):
/// - Bomb/rocket `reload_s` 180 s: §9.6.3 resolves unguided attacks once per
///   3-minute Tactical Turn; the engine uses that turn length as the
///   continuous-time re-attack cycle.
/// - Aircraft hp 35, cruise 420 kt, turn 8°/s, VP 14, surface-search radar +
///   visual fit, 12 chaff — mirrors the existing `striker` class.
/// - Pk 0.35 / 0.25 are the work-order values calibrated against the
///   Unguided Ordnance Attack Table (§9.6.5/§9.6.7, PDF p.107) for a nominal
///   low-altitude delivery; see [`delivery_modifier`].
///
/// Loadout note: `mounts: 1, ammo: 4` spawns four bomb rounds (`scenario.rs`
/// multiplies ammo by mounts) and keeps the Bomb branch's
/// `salvo = def.salvo.max(mounts.min(2))` at one bomb per pass, so four
/// passes release exactly four bombs. The rocket pod is one mount whose
/// single engagement releases `salvo = 4` rockets (§9.6.7 pairs).
pub fn db() -> Database {
    const UNGUIDED_DB_JSON: &str = r#"{
      "platforms": {
        "strike_attacker": {
          "id": "strike_attacker", "name": "Strike Attacker", "kind": "Aircraft",
          "hp": 35, "max_speed_kts": 500, "cruise_speed_kts": 420,
          "turn_rate_deg_s": 8.0, "rcs": "Small", "vp_value": 14,
          "endurance_s": 7200, "max_depth_m": 0,
          "sensors": ["radar_surf_sm", "visual"],
          "weapons": [
            {"weapon": "bomb_mk82", "mounts": 1, "ammo": 4},
            {"weapon": "rocket_57", "mounts": 1, "ammo": 1}
          ],
          "chaff": 12
        }
      },
      "weapons": {
        "bomb_mk82": {
          "id": "bomb_mk82", "name": "Mk 82 500 lb Bomb", "kind": "Bomb",
          "targets": {"surface": true, "land": true},
          "range_nm": 0.8, "speed_kts": 0, "pk": 0.35, "salvo": 1,
          "reload_s": 180, "damage": 30, "guidance": "BallisticGun"
        },
        "rocket_57": {
          "id": "rocket_57", "name": "57 mm Unguided Rocket", "kind": "Rocket",
          "targets": {"surface": true, "land": true},
          "range_nm": 3, "speed_kts": 0, "pk": 0.25, "salvo": 4,
          "reload_s": 180, "damage": 12, "guidance": "BallisticGun"
        }
      },
      "sensors": {}
    }"#;
    serde_json::from_str(UNGUIDED_DB_JSON).expect("unguided database extras must parse")
}

/// Release altitude/speed modifier for unguided ordnance (§9.6.1-§9.6.7,
/// RTS calibration).
///
/// - `< 300 m`: ×1.2
/// - `300..=600 m`: ×1.0
/// - `> 600 m`: ×0.7
/// - release speed `> 450 kt`: ×0.8 on top of the altitude band
///
/// The 300/600 m bands and 1.2/0.7/0.8 factors are estimated against the
/// rulebook's altitude-row and delivery-profile modifiers (§9.6.4-§9.6.5,
/// PDF pp.105-107); they are this engine's calibration, not published
/// tabletop numbers. The factor is exposed for scenario authors and future
/// wiring; `weapons.rs` currently adjudicates Bomb/Rocket with `def.pk`
/// alone (the gun-style range/aspect bands still apply), so the altitude
/// band is absorbed into the calibrated database `pk` rather than
/// multiplied at resolution time.
pub fn delivery_modifier(alt_m: f64, speed_kts: f64) -> f64 {
    let altitude_mult = if alt_m < LOW_ALT_BAND_M {
        LOW_ALT_MODIFIER
    } else if alt_m > HIGH_ALT_BAND_M {
        HIGH_ALT_MODIFIER
    } else {
        1.0
    };
    if speed_kts > FAST_RELEASE_KTS {
        altitude_mult * FAST_RELEASE_MODIFIER
    } else {
        altitude_mult
    }
}

/// Slant release-to-impact distance in nautical miles for a straight dive.
///
/// Geometry: the release altitude `alt_m` is the opposite side of the dive
/// angle, so the slant path is `alt_m / sin(dive_angle_deg)`. This gives the
/// radius a scenario author should clear around the aim point / dive path
/// when placing shipping, islands, or friendlies. It is straight-line dive
/// geometry and does not model gravity or the bomb's forward travel after
/// release. Dive angles above 90° are clamped to vertical; non-positive or
/// non-finite altitude/angle returns 0. A 0° (level) release has no dive
/// slant path and also returns 0; §9.6.4 level-bombing release ranges (6 nmi
/// at High, 3 nmi at Medium) are outside this helper.
///
/// Rules basis: §7.3 Weapons Danger Space (PDF p.76 / printed 7-1) covers
/// stray gunfire and missiles, while §9.6 (PDF p.105 / printed 9-4) says a
/// weapon that misses a ground target has no other effect. The engine does
/// NOT adjudicate live unguided friendly fire; this is a planning tool only
/// (the accepted simplification in this workstream).
pub fn danger_space_nm(alt_m: f64, dive_angle_deg: f64) -> f64 {
    if !alt_m.is_finite()
        || !dive_angle_deg.is_finite()
        || alt_m <= 0.0
        || dive_angle_deg <= 0.0
    {
        return 0.0;
    }
    let angle_deg = dive_angle_deg.min(90.0);
    let slant_m = alt_m / angle_deg.to_radians().sin();
    slant_m / 1852.0
}

/// Per-tick unguided-weapons behavior.
///
/// No state to advance: Bomb/Rocket are adjudicated instantly by
/// `weapons.rs::fire_control_scan` (no projectile entity), and the danger
/// space is a scenario-author helper, so no `RulesEvent` is emitted here.
pub fn step(_world: &mut World) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Guidance, WeaponKind};
    use crate::types::{PlatformKind, RcsClass};

    fn assert_close(got: f64, want: f64) {
        assert!((got - want).abs() < 1e-9, "got {got}, expected {want}");
    }

    #[test]
    fn db_entries_merge_into_built_in_and_validate() {
        let db = Database::built_in();
        db.validate().expect("merged built-in database must validate");

        let bomb = db.weapon("bomb_mk82").expect("bomb_mk82 must be merged");
        assert_eq!(bomb.kind, WeaponKind::Bomb);
        assert_eq!(bomb.guidance, Guidance::BallisticGun);
        assert_eq!(bomb.salvo, 1);
        assert_close(bomb.range_nm, 0.8);
        assert_close(bomb.damage, 30.0);
        assert_close(bomb.pk, 0.35);
        assert!(bomb.targets.surface && bomb.targets.land);
        assert!(!bomb.targets.air);

        let rocket = db.weapon("rocket_57").expect("rocket_57 must be merged");
        assert_eq!(rocket.kind, WeaponKind::Rocket);
        assert_eq!(rocket.guidance, Guidance::BallisticGun);
        assert_eq!(rocket.salvo, 4, "§9.6.7: rockets fire in two pairs");
        assert_close(rocket.range_nm, 3.0);
        assert_close(rocket.damage, 12.0);
        assert_close(rocket.pk, 0.25);
        assert!(rocket.targets.surface && rocket.targets.land);

        let ac = db
            .platform("strike_attacker")
            .expect("strike_attacker must be merged");
        assert_eq!(ac.kind, PlatformKind::Aircraft);
        assert_eq!(ac.rcs, RcsClass::Small);
        assert_close(ac.max_speed_kts, 500.0);
        assert_close(ac.endurance_s, 7200.0);

        let bomb_fit = ac
            .weapons
            .iter()
            .find(|w| w.weapon == "bomb_mk82")
            .expect("strike_attacker carries the Mk 82");
        assert_eq!((bomb_fit.mounts, bomb_fit.ammo), (1, 4));
        let rocket_fit = ac
            .weapons
            .iter()
            .find(|w| w.weapon == "rocket_57")
            .expect("strike_attacker carries the rocket pod");
        assert_eq!((rocket_fit.mounts, rocket_fit.ammo), (1, 1));
    }

    #[test]
    fn strike_attacker_spawns_with_the_intended_loadout() {
        use crate::geom::Vec2;
        use crate::scenario::{load_scenario, Scenario, UnitSpawn};
        use crate::types::{DayNight, Side};
        use crate::world::Weather;

        let sc = Scenario {
            id: "unguided-loadout".into(),
            name: "Unguided Loadout".into(),
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
            seed: 9,
            victory: Default::default(),
            units: vec![UnitSpawn {
                side: Side::Blue,
                class: "strike_attacker".into(),
                name: Some("TEST STRIKE".into()),
                pos: Vec2::new(0.0, 0.0),
                course_deg: 0.0,
                speed_kts: 420.0,
                depth_m: None,
                altitude_m: Some(300.0),
                waypoints: None,
                follow: None,
                sonar_active: false,
                fire_on: false,
            }],
        };
        let w = load_scenario(sc, Database::built_in()).expect("strike_attacker scenario loads");
        let e = w
            .entities
            .values()
            .find(|e| e.name == "TEST STRIKE")
            .expect("strike attacker spawned");
        let bombs = e
            .weapons
            .iter()
            .find(|m| m.weapon == "bomb_mk82")
            .expect("bomb fit spawned");
        let rockets = e
            .weapons
            .iter()
            .find(|m| m.weapon == "rocket_57")
            .expect("rocket fit spawned");
        // scenario.rs spawns `ammo * mounts` rounds; mounts=1 keeps the Bomb
        // branch's salvo at 1 so four passes release four bombs.
        assert_eq!((bombs.mounts, bombs.ammo), (1, 4));
        assert_eq!((rockets.mounts, rockets.ammo), (1, 1));
        assert_close(e.endurance_s.expect("aircraft has endurance"), 7200.0);
    }

    #[test]
    fn delivery_modifier_bands() {
        // Altitude bands (<300 m bonus, 300..=600 m neutral, >600 m penalty).
        assert_close(delivery_modifier(200.0, 400.0), 1.2);
        assert_close(delivery_modifier(299.9, 400.0), 1.2);
        assert_close(delivery_modifier(300.0, 400.0), 1.0);
        assert_close(delivery_modifier(600.0, 400.0), 1.0);
        assert_close(delivery_modifier(600.1, 400.0), 0.7);
        assert_close(delivery_modifier(2500.0, 400.0), 0.7);

        // Speed band is strictly greater than 450 kt.
        assert_close(delivery_modifier(200.0, 450.0), 1.2);
        assert_close(delivery_modifier(450.0, 400.0), 1.0);
        assert_close(delivery_modifier(450.0, 500.0), 0.8);

        // Combined altitude × speed.
        assert_close(delivery_modifier(200.0, 451.0), 0.96);
        assert_close(delivery_modifier(450.0, 451.0), 0.8);
        assert_close(delivery_modifier(900.0, 500.0), 0.56);

        // Lower and slower is always better than higher and faster.
        assert!(delivery_modifier(250.0, 400.0) > delivery_modifier(450.0, 400.0));
        assert!(delivery_modifier(450.0, 400.0) > delivery_modifier(900.0, 400.0));
        assert!(delivery_modifier(900.0, 400.0) > delivery_modifier(900.0, 500.0));
    }

    #[test]
    fn danger_space_is_slant_range_for_the_dive() {
        // Vertical dive: danger space = altitude.
        assert_close(danger_space_nm(1000.0, 90.0), 1000.0 / 1852.0);
        // 45° dive: alt / sin(45°) = alt * sqrt(2).
        assert_close(
            danger_space_nm(1000.0, 45.0),
            1000.0 * 2f64.sqrt() / 1852.0,
        );
        // 30° dive from 100 m: 200 m of slant path.
        assert_close(danger_space_nm(100.0, 30.0), 200.0 / 1852.0);

        // Shallower dives have a larger danger space.
        assert!(danger_space_nm(1000.0, 20.0) > danger_space_nm(1000.0, 45.0));
        assert!(danger_space_nm(1000.0, 45.0) > danger_space_nm(1000.0, 90.0));

        // Degenerate input yields no computed danger space.
        assert_eq!(danger_space_nm(0.0, 45.0), 0.0);
        assert_eq!(danger_space_nm(1000.0, 0.0), 0.0);
        assert_eq!(danger_space_nm(1000.0, -30.0), 0.0);
        assert_eq!(danger_space_nm(f64::NAN, 45.0), 0.0);
        // Angles beyond vertical clamp to vertical.
        assert_close(
            danger_space_nm(500.0, 120.0),
            danger_space_nm(500.0, 90.0),
        );
    }
}

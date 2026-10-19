//! Strike / land-attack depth: runway & base component state machines,
//! Tattletail regression scenario support.
//!
//! Hook: `step(world)` advances base component states stored on `entity.ext`
//! ("runway_ops" 0..1, "radar_ops", ...) scaling deck regeneration with
//! damage per the land-strike chapters.
//!
//! State model (RTS adaptation of the land-strike chapters):
//! - A base's runway and radar plants remain operable in proportion to the
//!   surviving hull fraction. `runway_ops` / `radar_ops` are recomputed from
//!   `hp / hp_max` every tick, so they fall as strike damage accumulates and
//!   recover if the hull is repaired by some future rule.
//! - Deck operations scale with the runway state: a landing/rearming cycle
//!   takes `base_cycle_s * (2 - runway_ops)` seconds — from the nominal time
//!   at full health up to twice that on a cratered runway. The base's nominal
//!   cycle is captured once in `ext["strike.base_cycle_s"]`; the live
//!   `DeckState::cycle_s` carries the scaled value used by `air::try_recover`.
//! - The first tick `runway_ops` drops below 0.5 emits one
//!   `RulesEvent{strike, runway_degraded}`. The crossing is latched in
//!   `ext["strike.runway_degraded_announced"]`, so a repaired-then-damaged
//!   runway does not spam the log; only the initial loss of >50% runway
//!   capacity is reported.
//!
//! Runway state is only tracked/announced for bases that actually own a deck
//! (an airfield); a naval station still carries `radar_ops` but has no runway
//! to degrade.

use crate::events::{EventKind, Visibility};
use crate::types::{EntityId, PlatformKind, Side};
use crate::world::World;

/// Runway operability below which the field is announced as degraded.
pub const RUNWAY_DEGRADED_THRESHOLD: f64 = 0.5;

const EXT_RUNWAY_OPS: &str = "runway_ops";
const EXT_RADAR_OPS: &str = "radar_ops";
const EXT_BASE_CYCLE: &str = "strike.base_cycle_s";
const EXT_DEGRADED_ANNOUNCED: &str = "strike.runway_degraded_announced";

/// Extra DB (runway/radar component ratings) — none needed: the RTS model
/// derives component health from the hull pool and the deck's nominal cycle.
pub fn db() -> crate::db::Database {
    crate::db::Database::default()
}

/// Per-tick base component behavior.
pub fn step(world: &mut World) {
    let ids: Vec<EntityId> = world
        .entities
        .values()
        .filter(|e| e.alive && e.kind == PlatformKind::Base)
        .map(|e| e.id)
        .collect();

    let mut degraded: Vec<(EntityId, Side, String, f64)> = Vec::new();

    for id in ids {
        let Some(e) = world.entity_mut(id) else { continue };
        let hp_frac = if e.hp_max > 0.0 {
            (e.hp / e.hp_max).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // Component plants follow the surviving hull.
        e.ext.insert(EXT_RUNWAY_OPS.to_string(), hp_frac);
        e.ext.insert(EXT_RADAR_OPS.to_string(), hp_frac);

        let Some(deck) = e.deck.as_mut() else { continue };
        let base_cycle = *e
            .ext
            .entry(EXT_BASE_CYCLE.to_string())
            .or_insert(deck.cycle_s.max(1.0));
        let scaled = base_cycle.max(1.0) * (2.0 - hp_frac);
        deck.cycle_s = scaled;

        let already = e
            .ext
            .get(EXT_DEGRADED_ANNOUNCED)
            .copied()
            .unwrap_or(0.0)
            >= 0.5;
        if hp_frac < RUNWAY_DEGRADED_THRESHOLD && !already {
            e.ext
                .insert(EXT_DEGRADED_ANNOUNCED.to_string(), 1.0);
            degraded.push((id, e.side, e.name.clone(), scaled));
        }
    }

    for (id, side, name, scaled_cycle) in degraded {
        let hp_frac = world
            .entity(id)
            .map(|e| if e.hp_max > 0.0 { (e.hp / e.hp_max).clamp(0.0, 1.0) } else { 1.0 })
            .unwrap_or(0.0);
        world.emit(
            EventKind::RulesEvent {
                domain: "strike".into(),
                rule: "runway_degraded".into(),
                side: Some(side),
                unit: Some(id),
                contact: None,
                detail: format!(
                    "{name} runway below {:.0}% operable (hull {:.0}%); deck cycle \
                     {scaled_cycle:.0} s = nominal x{:.2}",
                    RUNWAY_DEGRADED_THRESHOLD * 100.0,
                    hp_frac * 100.0,
                    2.0 - hp_frac
                ),
            },
            Visibility::Side(side),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Vec2;
    use crate::scenario::{load_scenario, Scenario, UnitSpawn};
    use crate::types::DayNight;
    use crate::world::Weather;

    fn base_world() -> World {
        let sc = Scenario {
            id: "strike-test".into(),
            name: "Strike Test".into(),
            description: String::new(),
            context: Default::default(),
            duration_s: 100_000,
            weather: Weather { visibility_nm: 20.0, jam: [0.0, 0.0], overcast: 0.0, rain: 0.0 },
            day_night: DayNight::Day,
            seed: 20251018,
            victory: Default::default(),
            units: vec![
                UnitSpawn {
                    side: Side::Red,
                    class: "airfield".into(),
                    name: Some("TEST AIRFIELD".into()),
                    pos: Vec2::new(0.0, 0.0),
                    course_deg: 0.0,
                    speed_kts: 0.0,
                    depth_m: None,
                    altitude_m: None,
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: false,
                },
                UnitSpawn {
                    side: Side::Red,
                    class: "navsta".into(),
                    name: Some("TEST NAVSTA".into()),
                    pos: Vec2::new(10.0, 0.0),
                    course_deg: 0.0,
                    speed_kts: 0.0,
                    depth_m: None,
                    altitude_m: None,
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: false,
                },
            ],
        };
        load_scenario(sc, crate::db::Database::built_in()).expect("load")
    }

    fn id_of(w: &World, name: &str) -> EntityId {
        w.entities
            .values()
            .find(|e| e.name == name)
            .map(|e| e.id)
            .unwrap_or_else(|| panic!("no entity named {name}"))
    }

    fn runway_degraded(w: &World) -> usize {
        w.events
            .iter()
            .filter(|e| {
                matches!(
                    &e.kind,
                    EventKind::RulesEvent { domain, rule, .. }
                        if domain == "strike" && rule == "runway_degraded"
                )
            })
            .count()
    }

    #[test]
    fn component_state_tracks_hp_and_scales_deck_cycle() {
        let mut w = base_world();
        let airfield = id_of(&w, "TEST AIRFIELD");

        // Full health: components operable, nominal 180 s deck cycle.
        step(&mut w);
        {
            let e = w.entity(airfield).unwrap();
            assert_eq!(e.ext.get(EXT_RUNWAY_OPS).copied(), Some(1.0));
            assert_eq!(e.ext.get(EXT_RADAR_OPS).copied(), Some(1.0));
            assert_eq!(e.deck.as_ref().unwrap().cycle_s, 180.0);
        }
        assert_eq!(runway_degraded(&w), 0);

        // 35% hull: component state follows, and a cycling deck now takes
        // 180 * (2 - 0.35) = 297 s.
        w.entity_mut(airfield).unwrap().hp = 0.35 * 400.0;
        step(&mut w);
        {
            let e = w.entity(airfield).unwrap();
            assert!((e.ext[EXT_RUNWAY_OPS] - 0.35).abs() < 1e-9);
            assert!((e.ext[EXT_RADAR_OPS] - 0.35).abs() < 1e-9);
            assert!((e.deck.as_ref().unwrap().cycle_s - 297.0).abs() < 1e-9);
        }
        assert_eq!(runway_degraded(&w), 1, "first crossing must be announced");

        // The first crossing is latched: repeated damage/repair cycles do not
        // re-announce, but the scaled cycle keeps tracking component health.
        w.entity_mut(airfield).unwrap().hp = 0.60 * 400.0;
        step(&mut w);
        assert!((w.entity(airfield).unwrap().deck.as_ref().unwrap().cycle_s - 252.0).abs() < 1e-9);
        w.entity_mut(airfield).unwrap().hp = 0.40 * 400.0;
        step(&mut w);
        assert_eq!(runway_degraded(&w), 1, "runway_degraded is a first-crossing event");
    }

    #[test]
    fn naval_station_tracks_radar_but_has_no_runway_event() {
        let mut w = base_world();
        let navsta = id_of(&w, "TEST NAVSTA");
        w.entity_mut(navsta).unwrap().hp = 0.20 * 500.0;
        step(&mut w);
        {
            let e = w.entity(navsta).unwrap();
            assert!((e.ext[EXT_RADAR_OPS] - 0.20).abs() < 1e-9);
            assert_eq!(e.ext.get(EXT_RUNWAY_OPS).copied(), Some(0.20));
            assert!(e.deck.is_none());
        }
        assert_eq!(
            runway_degraded(&w),
            0,
            "a base without a deck has no runway to degrade"
        );
    }
}

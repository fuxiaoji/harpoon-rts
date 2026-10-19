//! Torpedo guidance (§7.5): seeker generations, dual-speed, wire guidance,
//! wake homing.
//!
//! Rules source: `14-2.HpnV_Rules_1_Jan_26.pdf` (2026-01-01 edition).
//! - §7.5.1 Torpedo Seeker Generations, p. 7-4 (PDF 79), table visually
//!   verified: Gen 1 hit 40% / active acquisition 500 yd / passive 300 yd;
//!   Gen 2 60% / 1,000 / 500; Gen 3 70% / 2,000 / 1,000; Gen 4 80% /
//!   4,000 / 2,000. Acquisition cone is ±60°. `torp_mod0..2` map to
//!   Gen 1..3. The decoy-resistance multipliers (1.0 / 0.7 / 0.45) are
//!   fixed by the P1 work order; the table has no numeric decoy column.
//! - §7.5.4 Wire-Guided Torpedoes, pp. 7-4..7-5 (PDF 79-80): while the wire
//!   is intact the launch platform can update the torpedo's course, speed,
//!   depth and enable point. The official wire-break table is speed/maneuver
//!   based; the 15 nm leash and the sensor-cadence aim refresh below are the
//!   work order's RTS adaptation [ADAPTED].
//! - §7.5.5 Wake-Homing, p. 7-5 (PDF 80): a high-frequency active sonar
//!   detects the turbulent wake; the weapon is aimed to pass astern and then
//!   follows the wake. Acquisition itself lives in `weapons.rs`, which is
//!   outside this module's ownership, so the wake rule here publishes
//!   `RulesEvent{domain:"torpedo", rule:"wake_detected"}` the first time an
//!   armed WakeHome torpedo enters the target's wake band. The 0.3 nm band
//!   is fixed by the work order [ADAPTED].
//! - §7.5.7 Dual-Speed Torpedoes, p. 7-5 (PDF 80): slower settings trade
//!   speed for range, and a homing torpedo automatically accelerates to its
//!   fastest setting once it acquires (§3.5 Torpedo Movement, p. 3-10 /
//!   PDF 32). The 0.6x economy setting is the work order's RTS adaptation
//!   [ADAPTED].
//!
//! The P1 work-order text cites dual-speed as §7.5.4 and wire guidance as
//! §7.5.7; in the 2026-01-01 edition the numbers are reversed (§7.5.4 is
//! Wire-Guided, §7.5.7 is Dual-Speed), which is what this module follows.
//!
//! Registered from `rules::step` (one-line integration fix: the skeleton
//! registry did not yet list `torpedoes::step`).

use crate::db::{Database, Guidance, WeaponDef, WeaponKind};
use crate::events::{EventKind, Visibility};
use crate::geom::{normalize_deg, Vec2};
use crate::types::{EntityId, Side};
use crate::world::{World, SENSOR_INTERVAL_TICKS};

/// Dual-speed economy setting as a fraction of full speed (work order
/// adaptation of §7.5.7, PDF p. 80).
const ECONOMY_SPEED_FACTOR: f64 = 0.6;
/// Maximum wire length in this RTS model (work order; the official rule uses
/// a speed/maneuver wire-break table instead of a fixed range, §7.5.4).
const WIRE_MAX_RANGE_NM: f64 = 15.0;
/// A firing solution needs contact quality strictly above this value
/// (work order; matches the engine's manual-fire quality gate).
const WIRE_MIN_CONTACT_QUALITY: f64 = 0.3;
/// Half-width of the target's wake track (work order).
const WAKE_BAND_NM: f64 = 0.3;

/// Scratch-state key on the torpedo entity: 1.0 while the wire is feeding
/// the pre-enable aim point, 0.0 when the link is broken or unusable.
const EXT_WIRE_GUIDED: &str = "wire_guided";
/// Scratch-state key on the torpedo entity: latched once the armed weapon has
/// entered the target's wake band, so the event fires once per enable.
const EXT_WAKE_DETECTED: &str = "wake_detected";

/// Extra database entries: the three torpedo seeker generations.
///
/// §7.5.1 (p. 7-4 / PDF 79) supplies `pk` (the table's "Chance of a hit")
/// and the active acquisition range, converted 500/1,000/2,000 yd to
/// 0.25/0.5/1.0 nm. The engine uses `enable_range_nm` as the straight-run
/// distance before the seeker goes live, so the acquisition range is the
/// closest available data-driven stand-in [ADAPTED].
///
/// Speed, range and damage are not part of the seeker table; they are
/// ESTIMATED heavyweight-torpedo values on the `db_data.rs` damage scale
/// (heavyweight ≈ 200+ DP) and can be replaced by Annex F platform data
/// without touching the rules code.
pub fn db() -> Database {
    const TORPEDO_DB_JSON: &str = r#"{
      "platforms": {},
      "weapons": {
        "torp_mod0": {
          "id": "torp_mod0", "name": "Seeker Gen 1 Torpedo", "kind": "Torpedo",
          "targets": {"surface": true, "sub": true},
          "range_nm": 6, "speed_kts": 30, "enable_range_nm": 0.25,
          "pk": 0.40, "salvo": 1, "reload_s": 30, "damage": 200,
          "guidance": "TorpedoActive"
        },
        "torp_mod1": {
          "id": "torp_mod1", "name": "Seeker Gen 2 Torpedo", "kind": "Torpedo",
          "targets": {"surface": true, "sub": true},
          "range_nm": 10, "speed_kts": 45, "enable_range_nm": 0.5,
          "pk": 0.60, "salvo": 1, "reload_s": 30, "damage": 240,
          "guidance": "TorpedoActive"
        },
        "torp_mod2": {
          "id": "torp_mod2", "name": "Seeker Gen 3 Torpedo", "kind": "Torpedo",
          "targets": {"surface": true, "sub": true},
          "range_nm": 16, "speed_kts": 55, "enable_range_nm": 1.0,
          "pk": 0.70, "salvo": 1, "reload_s": 30, "damage": 280,
          "guidance": "TorpedoActive"
        }
      },
      "sensors": {}
    }"#;
    serde_json::from_str(TORPEDO_DB_JSON).expect("torpedo database extras must parse")
}

// ---------------------------------------------------------------------------
// Seeker generations
// ---------------------------------------------------------------------------

/// True for projectile torpedoes (name or id carries "torp").
fn is_torpedo_def(def: &WeaponDef) -> bool {
    matches!(def.kind, WeaponKind::Torpedo | WeaponKind::Lwt)
        && (def.id.to_ascii_lowercase().contains("torp")
            || def.name.to_ascii_lowercase().contains("torp"))
}

/// Model index encoded in a torpedo id (`torp_mod0` -> 0, `torp_mod1` -> 1,
/// `torp_mod2` -> 2). Non-torpedoes and legacy ids return `None`.
fn torpedo_model_index(def: &WeaponDef) -> Option<u8> {
    if !is_torpedo_def(def) {
        return None;
    }
    let id = def.id.to_ascii_lowercase();
    let (_, tail) = id.rsplit_once("mod")?;
    if tail.is_empty() || !tail.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    tail.parse::<u8>().ok()
}

/// Decoy seduction multiplier for a torpedo seeker (§7.5.1 generations).
///
/// `weapons.rs` multiplies its 55% base seduction chance by this value, so
/// lower means harder to decoy. The 1.0 / 0.7 / 0.45 ladder is fixed by the
/// P1 work order (the 2026-01-01 table states only that resistance improves
/// with generation, without a numeric column).
pub fn decoy_resistance(def: &WeaponDef) -> f64 {
    match torpedo_model_index(def) {
        Some(0) => 1.0,
        Some(1) => 0.70,
        Some(2) => 0.45,
        _ => 1.0,
    }
}

fn is_dual_speed_torpedo(def: &WeaponDef) -> bool {
    torpedo_model_index(def) == Some(2)
}

fn is_wire_guided_torpedo(def: &WeaponDef) -> bool {
    torpedo_model_index(def) == Some(2)
}

// ---------------------------------------------------------------------------
// Per-tick pass
// ---------------------------------------------------------------------------

/// Per-tick torpedo guidance pass: dual-speed cruising, wire updates, and
/// wake-band reporting.
pub fn step(world: &mut World) {
    apply_dual_speed(world);
    update_wire_guidance(world);
    emit_wake_detections(world);
}

/// §7.5.7 dual-speed: a `torp_mod2` runs at 0.6x full speed until its seeker
/// enables, then returns to full speed (a homing torpedo accelerates on
/// acquisition, p. 7-5; §3.5 p. 3-10).
fn apply_dual_speed(world: &mut World) {
    let mut changes: Vec<(EntityId, f64)> = Vec::new();
    for e in world.entities.values() {
        if !e.alive {
            continue;
        }
        let Some(data) = e.projectile.as_ref() else { continue };
        let Some(def) = world.db.weapons.get(&data.weapon) else { continue };
        if !is_dual_speed_torpedo(def) {
            continue;
        }
        let ordered = if data.enabled {
            def.speed_kts
        } else {
            def.speed_kts * ECONOMY_SPEED_FACTOR
        };
        if (e.speed_kts - ordered).abs() > 1e-9 {
            changes.push((e.id, ordered));
        }
    }
    for (id, speed) in changes {
        if let Some(e) = world.entity_mut(id) {
            e.speed_kts = speed;
            e.ordered_speed_kts = speed;
        }
    }
}

/// §7.5.4 wire guidance: while the weapon is still pre-enable, a live
/// launcher inside the leash with a usable firing solution feeds the current
/// contact estimate into the torpedo's aim point once per sensor cadence.
/// Sinking the launcher or opening beyond the leash breaks the wire.
fn update_wire_guidance(world: &mut World) {
    if world.tick % SENSOR_INTERVAL_TICKS != 0 {
        return;
    }

    // command = None -> link broken/unusable; Some(point) -> steer to point.
    let mut commands: Vec<(EntityId, Option<Vec2>)> = Vec::new();
    for e in world.entities.values() {
        if !e.alive {
            continue;
        }
        let Some(data) = e.projectile.as_ref() else { continue };
        if data.enabled {
            continue; // wire control is modeled only before seeker enable
        }
        let Some(def) = world.db.weapons.get(&data.weapon) else { continue };
        if !is_wire_guided_torpedo(def) {
            continue;
        }

        let wire_ok = world
            .entities
            .get(&data.launched_by)
            .map(|shooter| shooter.alive && shooter.pos.range_to(e.pos) < WIRE_MAX_RANGE_NM)
            .unwrap_or(false);
        if !wire_ok {
            commands.push((e.id, None));
            continue;
        }

        let Some(target_id) = data.target else {
            commands.push((e.id, None));
            continue;
        };
        // The engine's knowledge is per-side (no per-shooter sensor picture),
        // so "the launcher holds the contact" is read from its side's book.
        let contact = world.sides[data.launched_by_side.index()]
            .contacts
            .values()
            .find(|c| {
                c.about == target_id
                    && !c.stale
                    && c.quality > WIRE_MIN_CONTACT_QUALITY
            });
        match contact {
            Some(c) => commands.push((e.id, Some(c.estimate_at(world.tick)))),
            None => commands.push((e.id, None)),
        }
    }

    for (id, aim) in commands {
        let Some(e) = world.entity_mut(id) else { continue };
        match aim {
            Some(point) => {
                if let Some(data) = e.projectile.as_mut() {
                    data.aim = point;
                }
                e.ext.insert(EXT_WIRE_GUIDED.into(), 1.0);
            }
            None => {
                e.ext.insert(EXT_WIRE_GUIDED.into(), 0.0);
            }
        }
    }
}

/// §7.5.5 wake homing: report the first entry of an armed WakeHome torpedo
/// into the target's wake band. Acquisition probability is applied in
/// `weapons.rs`; this event is the assertion hook for the wake rule.
fn emit_wake_detections(world: &mut World) {
    struct WakeHit {
        torpedo: EntityId,
        side: Side,
        name: String,
        target_name: String,
        offset_nm: f64,
    }

    let mut hits: Vec<WakeHit> = Vec::new();
    let mut resets: Vec<EntityId> = Vec::new();

    for e in world.entities.values() {
        if !e.alive {
            continue;
        }
        let Some(data) = e.projectile.as_ref() else { continue };
        let Some(def) = world.db.weapons.get(&data.weapon) else { continue };
        if def.guidance != Guidance::WakeHome {
            continue;
        }
        let latched = e.ext.get(EXT_WAKE_DETECTED).copied().unwrap_or(0.0) >= 1.0;
        if !data.enabled {
            // Re-arm the latch if a weapon ever goes cold again; in the
            // current weapons loop `enabled` only transitions once.
            if latched {
                resets.push(e.id);
            }
            continue;
        }
        if latched {
            continue;
        }
        let Some(target) = data.target.and_then(|t| world.entities.get(&t)) else { continue };
        if !target.alive {
            continue;
        }
        let offset = wake_band_offset_nm(e.pos, target.pos, target.course_deg);
        if offset <= WAKE_BAND_NM {
            hits.push(WakeHit {
                torpedo: e.id,
                side: e.side,
                name: e.name.clone(),
                target_name: target.name.clone(),
                offset_nm: offset,
            });
        }
    }

    for id in resets {
        if let Some(e) = world.entity_mut(id) {
            e.ext.insert(EXT_WAKE_DETECTED.into(), 0.0);
        }
    }

    for hit in hits {
        if let Some(e) = world.entity_mut(hit.torpedo) {
            e.ext.insert(EXT_WAKE_DETECTED.into(), 1.0);
        }
        world.emit(
            EventKind::RulesEvent {
                domain: "torpedo".into(),
                rule: "wake_detected".into(),
                side: Some(hit.side),
                unit: Some(hit.torpedo),
                contact: None,
                detail: format!(
                    "{} detected the wake of {} ({:.2} nm off the wake centerline)",
                    hit.name, hit.target_name, hit.offset_nm
                ),
            },
            Visibility::Side(hit.side),
        );
    }
}

/// Perpendicular distance from `torpedo_pos` to the target's wake centerline
/// (the astern ray from the target along its course), or `f64::MAX` when the
/// weapon is forward of the target. A stopped target still has a course ray
/// in this simplified model; no wake-persistence limit is applied.
fn wake_band_offset_nm(torpedo_pos: Vec2, target_pos: Vec2, target_course_deg: f64) -> f64 {
    let astern = Vec2::from_bearing(normalize_deg(target_course_deg + 180.0));
    let rel = torpedo_pos.sub(target_pos);
    let along = rel.x * astern.x + rel.y * astern.y;
    if along <= 0.0 {
        return f64::MAX;
    }
    (rel.x * -astern.y + rel.y * astern.x).abs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{load_scenario, Scenario, UnitSpawn};
    use crate::types::{ContactId, DayNight, PlatformKind};
    use crate::weapons;
    use crate::world::{Contact, Weather, WeaponMount};

    fn scenario() -> Scenario {
        Scenario {
            id: "torpedo-test".into(),
            name: "Torpedo rules test".into(),
            description: String::new(),
            context: Default::default(),
            duration_s: 3_600,
            weather: Weather {
                visibility_nm: 12.0,
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
                    class: "ssk".into(),
                    name: Some("BLUE SSK".into()),
                    pos: Vec2::new(0.0, 0.0),
                    course_deg: 90.0,
                    speed_kts: 6.0,
                    depth_m: Some(30.0),
                    altitude_m: None,
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: false,
                },
                UnitSpawn {
                    side: Side::Red,
                    class: "ddg_aegis".into(),
                    name: Some("RED DDG".into()),
                    pos: Vec2::new(10.0, 0.0),
                    course_deg: 0.0,
                    speed_kts: 18.0,
                    depth_m: None,
                    altitude_m: None,
                    waypoints: None,
                    follow: None,
                    sonar_active: false,
                    fire_on: false,
                },
            ],
        }
    }

    fn world() -> World {
        load_scenario(scenario(), Database::built_in()).expect("test scenario loads")
    }

    fn id_of(w: &World, name: &str) -> EntityId {
        w.entities
            .values()
            .find(|e| e.name == name)
            .map(|e| e.id)
            .unwrap_or_else(|| panic!("no entity named {name}"))
    }

    fn arm(w: &mut World, unit: EntityId, weapon: &str) {
        w.entity_mut(unit).unwrap().weapons.push(WeaponMount {
            weapon: weapon.into(),
            mounts: 1,
            ammo: 4,
            cooldown_s: 0.0,
        });
    }

    /// Fire through the real launch path so the projectile carries normal
    /// aim/live_s/enabled defaults.
    fn launch_torpedo(w: &mut World, shooter: EntityId, target: EntityId, weapon: &str) -> EntityId {
        let def = w.db.weapons.get(weapon).expect("weapon in db").clone();
        weapons::launch(w, shooter, &def, Some(target), None).expect("torpedo launch")
    }

    fn seed_contact(
        w: &mut World,
        side: Side,
        about: EntityId,
        estimate: Vec2,
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
                category: PlatformKind::Ship,
                class_name: None,
                estimate,
                est_course_deg: 0.0,
                est_speed_kts: 0.0,
                bearing_only: false,
                quality,
                last_update_tick: w.tick,
                first_tick: w.tick,
                stale: false,
                sensors: vec!["seeded".into()],
                offset: Vec2::zero(),
                announced: false,
            },
        );
        cid
    }

    fn wake_events(w: &World) -> usize {
        w.events
            .iter()
            .filter(|e| {
                matches!(
                    &e.kind,
                    EventKind::RulesEvent { domain, rule, .. }
                        if domain == "torpedo" && rule == "wake_detected"
                )
            })
            .count()
    }

    #[test]
    fn seeker_generations_and_decoy_resistance() {
        let db = Database::built_in();

        // §7.5.1 table (p. 7-4): hit chance and active acquisition range by
        // generation; the database must carry them verbatim.
        for (id, pk, enable_nm, resistance) in [
            ("torp_mod0", 0.40, 0.25, 1.0),
            ("torp_mod1", 0.60, 0.50, 0.70),
            ("torp_mod2", 0.70, 1.00, 0.45),
        ] {
            let def = db.weapon(id).expect("generation torpedo in db");
            assert_eq!(def.kind, WeaponKind::Torpedo, "{id} kind");
            assert_eq!(def.guidance, Guidance::TorpedoActive, "{id} guidance");
            assert!((def.pk - pk).abs() < 1e-9, "{id} pk {}", def.pk);
            assert!(
                (def.enable_range_nm - enable_nm).abs() < 1e-9,
                "{id} enable range {}",
                def.enable_range_nm
            );
            assert!(
                (decoy_resistance(def) - resistance).abs() < 1e-9,
                "{id} decoy resistance {}",
                decoy_resistance(def)
            );
        }

        // Legacy torpedoes and non-torpedo projectiles keep the baseline.
        let legacy = db.weapon("torp_wake").expect("legacy wake torpedo");
        assert_eq!(decoy_resistance(legacy), 1.0);
        let ascm = db.weapon("ascm_std").expect("ascm");
        assert_eq!(decoy_resistance(ascm), 1.0);
    }

    #[test]
    fn dual_speed_torpedo_cruises_economy_until_enabled() {
        let mut w = world();
        let sub = id_of(&w, "BLUE SSK");
        let ddg = id_of(&w, "RED DDG");
        arm(&mut w, sub, "torp_mod2");
        arm(&mut w, sub, "torp_mod1");

        let game_torp = launch_torpedo(&mut w, sub, ddg, "torp_mod2");
        let gen2_torp = launch_torpedo(&mut w, sub, ddg, "torp_mod1");
        let full = w.db.weapon("torp_mod2").unwrap().speed_kts;
        let gen2_full = w.db.weapon("torp_mod1").unwrap().speed_kts;

        // Launch starts at full speed; the first step is where cruising begins.
        assert!((w.entity(game_torp).unwrap().speed_kts - full).abs() < 1e-9);
        step(&mut w);
        let economy = w.entity(game_torp).unwrap().speed_kts;
        assert!(
            (economy - full * ECONOMY_SPEED_FACTOR).abs() < 1e-9,
            "economy speed {economy}, full {full}"
        );
        // Idempotent on the following ticks.
        step(&mut w);
        assert!((w.entity(game_torp).unwrap().speed_kts - full * ECONOMY_SPEED_FACTOR).abs() < 1e-9);
        // A Gen-2 (single-speed) torpedo is untouched by the dual-speed rule.
        assert!((w.entity(gen2_torp).unwrap().speed_kts - gen2_full).abs() < 1e-9);

        // Seeker enable restores full attack speed.
        w.entity_mut(game_torp)
            .unwrap()
            .projectile
            .as_mut()
            .unwrap()
            .enabled = true;
        step(&mut w);
        let attack = w.entity(game_torp).unwrap().speed_kts;
        assert!(
            (attack - full).abs() < 1e-9,
            "enable must restore full speed, got {attack}"
        );
    }

    #[test]
    fn wire_guidance_updates_aim_then_breaks() {
        let mut w = world();
        let sub = id_of(&w, "BLUE SSK");
        let ddg = id_of(&w, "RED DDG");
        arm(&mut w, sub, "torp_mod2");
        arm(&mut w, sub, "torp_mod1");
        let torpedo = launch_torpedo(&mut w, sub, ddg, "torp_mod2");
        let legacy = launch_torpedo(&mut w, sub, ddg, "torp_mod1");

        let cid = seed_contact(&mut w, Side::Blue, ddg, Vec2::new(4.0, 9.0), 0.60);
        let launch_aim = w.entity(torpedo).unwrap().projectile.as_ref().unwrap().aim;

        // Non-sensor tick: no aim refresh.
        w.tick = SENSOR_INTERVAL_TICKS - 1;
        step(&mut w);
        assert_eq!(
            w.entity(torpedo).unwrap().projectile.as_ref().unwrap().aim,
            launch_aim,
            "wire aim must only refresh on the sensor cadence"
        );
        assert!(w
            .entity(torpedo)
            .unwrap()
            .ext
            .get(EXT_WIRE_GUIDED)
            .copied()
            .unwrap_or(0.0)
            .abs()
            < 1e-9);

        // Sensor tick: aim follows the contact estimate and the flag latches.
        w.tick = SENSOR_INTERVAL_TICKS;
        step(&mut w);
        let expected = w.sides[Side::Blue.index()].contacts[&cid].estimate_at(w.tick);
        assert_eq!(
            w.entity(torpedo).unwrap().projectile.as_ref().unwrap().aim,
            expected
        );
        assert_eq!(w.entity(torpedo).unwrap().ext[EXT_WIRE_GUIDED], 1.0);
        // A Gen-2 torpedo is not wire-guided.
        assert!(!w.entity(legacy).unwrap().ext.contains_key(EXT_WIRE_GUIDED));

        // Quality gate is strict: 0.30 does not hold a firing solution.
        {
            let c = w.sides[Side::Blue.index()].contacts.get_mut(&cid).unwrap();
            c.quality = 0.30;
            c.estimate = Vec2::new(1.0, 1.0);
        }
        w.tick = 2 * SENSOR_INTERVAL_TICKS;
        step(&mut w);
        assert_eq!(
            w.entity(torpedo).unwrap().projectile.as_ref().unwrap().aim,
            expected,
            "a sub-threshold contact must not steer the torpedo"
        );
        assert_eq!(w.entity(torpedo).unwrap().ext[EXT_WIRE_GUIDED], 0.0);

        // 0.31 resumes guidance.
        {
            let c = w.sides[Side::Blue.index()].contacts.get_mut(&cid).unwrap();
            c.quality = 0.31;
            c.estimate = Vec2::new(7.0, 2.0);
        }
        w.tick = 3 * SENSOR_INTERVAL_TICKS;
        step(&mut w);
        let expected = w.sides[Side::Blue.index()].contacts[&cid].estimate_at(w.tick);
        assert_eq!(
            w.entity(torpedo).unwrap().projectile.as_ref().unwrap().aim,
            expected
        );
        assert_eq!(w.entity(torpedo).unwrap().ext[EXT_WIRE_GUIDED], 1.0);

        // Launcher sunk: the wire is cut.
        w.entity_mut(sub).unwrap().alive = false;
        w.tick = 4 * SENSOR_INTERVAL_TICKS;
        step(&mut w);
        assert_eq!(w.entity(torpedo).unwrap().ext[EXT_WIRE_GUIDED], 0.0);
        assert_eq!(
            w.entity(torpedo).unwrap().projectile.as_ref().unwrap().aim,
            expected,
            "a cut wire leaves the last aim point untouched"
        );

        // Launcher alive but beyond the 15 nm leash: wire cut.
        w.entity_mut(sub).unwrap().alive = true;
        w.entity_mut(torpedo).unwrap().pos = Vec2::new(16.0, 0.0);
        w.tick = 5 * SENSOR_INTERVAL_TICKS;
        step(&mut w);
        assert_eq!(w.entity(torpedo).unwrap().ext[EXT_WIRE_GUIDED], 0.0);
    }

    #[test]
    fn wake_homing_reports_first_entry_per_enable() {
        let mut w = world();
        let sub = id_of(&w, "BLUE SSK");
        let ddg = id_of(&w, "RED DDG");
        arm(&mut w, sub, "torp_wake");
        let torpedo = launch_torpedo(&mut w, sub, ddg, "torp_wake");

        // Put the target on a due-north track and the torpedo one mile astern.
        {
            let t = w.entity_mut(ddg).unwrap();
            t.pos = Vec2::new(0.0, 0.0);
            t.course_deg = 0.0;
        }
        w.entity_mut(torpedo).unwrap().pos = Vec2::new(0.0, -1.0);

        // Seeker cold: no event even though the weapon is in the band.
        step(&mut w);
        assert_eq!(wake_events(&w), 0, "a cold seeker cannot detect a wake");

        // Enabled but 0.5 nm off the wake centerline: outside the 0.3 nm band.
        {
            let p = w.entity_mut(torpedo).unwrap();
            p.projectile.as_mut().unwrap().enabled = true;
            p.pos = Vec2::new(0.5, -1.0);
        }
        step(&mut w);
        assert_eq!(wake_events(&w), 0, "0.5 nm abeam is outside the wake band");

        // Inside the band: one event, latched.
        w.entity_mut(torpedo).unwrap().pos = Vec2::new(0.2, -1.0);
        step(&mut w);
        assert_eq!(wake_events(&w), 1);
        assert_eq!(w.entity(torpedo).unwrap().ext[EXT_WAKE_DETECTED], 1.0);

        // Dedup: further ticks in the band do not re-announce.
        step(&mut w);
        assert_eq!(wake_events(&w), 1);

        // Re-arm (only relevant if the seeker ever goes cold again) and
        // check the forward half-plane is not treated as wake.
        w.entity_mut(torpedo)
            .unwrap()
            .projectile
            .as_mut()
            .unwrap()
            .enabled = false;
        step(&mut w);
        assert_eq!(w.entity(torpedo).unwrap().ext[EXT_WAKE_DETECTED], 0.0);
        {
            let p = w.entity_mut(torpedo).unwrap();
            p.projectile.as_mut().unwrap().enabled = true;
            p.pos = Vec2::new(0.0, 1.0); // ahead of the target
        }
        step(&mut w);
        assert_eq!(wake_events(&w), 1, "ahead of the bow is not the wake");

        w.entity_mut(torpedo).unwrap().pos = Vec2::new(0.1, -1.0);
        step(&mut w);
        assert_eq!(wake_events(&w), 2, "a fresh enable re-arms the wake report");

        // The event names the firing side and the torpedo.
        let ev = w
            .events
            .iter()
            .find(|e| {
                matches!(
                    &e.kind,
                    EventKind::RulesEvent { domain, rule, .. }
                        if domain == "torpedo" && rule == "wake_detected"
                )
            })
            .expect("wake_detected event");
        match &ev.kind {
            EventKind::RulesEvent { side, unit, .. } => {
                assert_eq!(*side, Some(Side::Blue));
                assert_eq!(*unit, Some(torpedo));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn step_is_wired_into_the_tick_loop() {
        // Guards the one-line `rules::step` registration: a real tick must
        // slow the dual-speed torpedo and run the wire pass on its cadence.
        let mut w = world();
        let sub = id_of(&w, "BLUE SSK");
        let ddg = id_of(&w, "RED DDG");
        arm(&mut w, sub, "torp_mod2");
        let torpedo = launch_torpedo(&mut w, sub, ddg, "torp_mod2");
        let full = w.db.weapon("torp_mod2").unwrap().speed_kts;

        crate::sim::tick(&mut w, &[]);
        assert!(
            (w.entity(torpedo).unwrap().speed_kts - full * ECONOMY_SPEED_FACTOR).abs() < 1e-9,
            "rules::step must be wired into sim::tick"
        );
        assert!(!w.entity(torpedo).unwrap().projectile.as_ref().unwrap().enabled);

        // Seed the firing solution and run to the next sensor pass. The aim
        // update happens in rules::step before sensors.rs runs, so this only
        // checks that the loop reaches the wire rule.
        seed_contact(&mut w, Side::Blue, ddg, Vec2::new(4.0, 9.0), 0.60);
        for _ in 0..3 {
            crate::sim::tick(&mut w, &[]);
        }
        assert_eq!(w.tick, 4);
        assert_eq!(w.entity(torpedo).unwrap().ext[EXT_WIRE_GUIDED], 1.0);
    }

    #[test]
    fn wake_band_geometry_is_astern_only() {
        // Direct geometry check: narrow astern sector, perpendicular distance.
        assert!(
            (wake_band_offset_nm(Vec2::new(0.0, -2.0), Vec2::zero(), 0.0) - 0.0).abs() < 1e-12
        );
        assert!(
            (wake_band_offset_nm(Vec2::new(0.29, -2.0), Vec2::zero(), 0.0) - 0.29).abs() < 1e-12
        );
        assert_eq!(
            wake_band_offset_nm(Vec2::new(0.0, 2.0), Vec2::zero(), 0.0),
            f64::MAX
        );
        // Course 90 (east): the wake trails west, so a torpedo to the west is
        // in the band and one to the east is not.
        assert!(
            (wake_band_offset_nm(Vec2::new(-2.0, 0.1), Vec2::zero(), 90.0) - 0.1).abs() < 1e-12
        );
        assert_eq!(
            wake_band_offset_nm(Vec2::new(2.0, 0.0), Vec2::zero(), 90.0),
            f64::MAX
        );
    }
}

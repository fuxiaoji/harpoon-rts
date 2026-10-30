//! ZCODE-NUCLEAR-NEXT-02 candidate, revision 2 (root review 2026-10-28 applied:
//! `1u8 +` arithmetic, seed attempts capped at 8, real-flight interception via up to
//! 25 actual advance_projectiles calls, detonation cause read from detail.burst.cause,
//! unarmed control added, no runtime pos/kind injection anywhere).
//! Suggested target: crates/harpoon-core/tests/. Unregistered artifact; root owns
//! registration, the normal-compile source-red run and any fix authorization.
//!
//! Source (English2026 PDF132/printed 14-6, §14.1.8 Fuzing, visually reviewed 2026-10-28):
//!   "Once fuzed, nuclear warheads have a 50% chance of detonating even if the weapon
//!    carrying them is destroyed."
//! Actual unclosed consumer: the real defense SAM packet terminal
//! (weapons.rs::source_air_terminal) that really destroys an incoming armed nuclear
//! missile retires it through weapons.rs::remove_projectile (line ~1071) with
//! nuclear2026::Removal::Unresolved — no `14.1.8.carrier_destroyed` D100 and no burst.
//! damage.rs::sink_by (line ~655) already routes projectile kills to
//! Removal::CarrierDestroyed, but that path is unreachable from defense kills.
//! Root fix scope (agreed): only the source_air_terminal branch where the target is
//! really destroyed; neutral removals (runout/decoy/interference/aim-pass) stay
//! Unresolved, and the roll stays gated on f.armed.
//!
//! Laboratory fits only (`scenario_assumption:`), no real nuclear Annex fit exists:
//! `us_essm_i` is the aaw2026 v4 fixture-injected ESSM Block I installed on us_ford,
//! the same accepted environment as tests/nuclear_batch_resume_20261025.rs. All unit
//! motion comes from the real simulation loop; nothing is teleported.
use harpoon_core::rules::{aaw2026 as a, nuclear2026 as n};
use harpoon_core::{Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::json;

const MAX_SEEDS: usize = 8;
const MAX_ADVANCES: usize = 25;

fn fixture_armed(seed: u64) -> (World, EntityId, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"nuclear-carrier-destroyed-source",
        "name":"scenario_assumption: nuclear carrier-destruction laboratory",
        "duration_s":10000,"seed":seed,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "nuclear_2026":{"source":"scenario_assumption: laboratory delivery, not a historical nuclear ESSM variant",
                "units":{"LAB":{"us_essm_i":{"source":"scenario_assumption: laboratory-only delivery","source_variant":"scenario_assumption:carrier_lab","allowed_yields_kt":[1,5],"selected_yield_kt":1,"medium":"air","fuze":"free_fall","authorized":true}}}}},
        "units":[
            {"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_052d","name":"DEFNDER","pos":[0,10],"speed_kts":0,"fire_on":false}
        ]})).unwrap();
    let w = harpoon_core::load_scenario(sc, db).unwrap();
    let b = w.entities.values().find(|e| e.name == "LAB").unwrap().id;
    let d = w.entities.values().find(|e| e.name == "DEFNDER").unwrap().id;
    (w, b, d)
}

/// Unarmed control: the incoming warhead uses the printed two-Increment (one minute)
/// time-to-target arming rule against a far aim point, so it is still NOT fuzed when
/// a nearby defender kills it seconds into the flight.
fn fixture_unarmed(seed: u64) -> (World, EntityId, EntityId, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"nuclear-carrier-unarmed-control",
        "name":"scenario_assumption: nuclear unarmed-carrier control laboratory",
        "duration_s":10000,"seed":seed,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "nuclear_2026":{"source":"scenario_assumption: laboratory delivery, not a historical nuclear ESSM variant",
                "units":{"LAB":{"us_essm_i":{"source":"scenario_assumption: laboratory-only delivery","source_variant":"scenario_assumption:carrier_lab","allowed_yields_kt":[1,5],"selected_yield_kt":1,"medium":"air","fuze":"time_to_target","authorized":true}}}}},
        "units":[
            {"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_052d","name":"AIMPT","pos":[0,45],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_052d","name":"SHOOTER","pos":[0,1],"speed_kts":0,"fire_on":false}
        ]})).unwrap();
    let w = harpoon_core::load_scenario(sc, db).unwrap();
    let b = w.entities.values().find(|e| e.name == "LAB").unwrap().id;
    let a0 = w.entities.values().find(|e| e.name == "AIMPT").unwrap().id;
    let s = w.entities.values().find(|e| e.name == "SHOOTER").unwrap().id;
    (w, b, a0, s)
}

fn detonation_causes(w: &World) -> Vec<String> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "14.1.8.detonation" => Some(
                serde_json::from_str::<serde_json::Value>(detail).unwrap()["burst"]["cause"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            ),
            _ => None,
        })
        .collect()
}

fn carrier_destroyed_trace_present(w: &World) -> bool {
    w.events.iter().any(|e| {
        matches!(
            &e.kind,
            EventKind::RulesEvent { rule, .. } if rule == "14.1.8.carrier_destroyed"
        )
    })
}

/// Drives the real defense packet to a real intercept: up to MAX_ADVANCES actual
/// advance_projectiles calls until the aaw outcome exists. No pos/kind injection.
fn run_defense_to_outcome(
    w: &mut World,
    d: EntityId,
    m: EntityId,
) -> Result<(), &'static str> {
    let hhq9 = w.db.weapons["cn_hhq9"].clone();
    let Some(q) = harpoon_core::weapons::launch(w, d, &hhq9, Some(m), None) else {
        return Err("interceptor launch failed");
    };
    let attack = a::MissileAttack {
        ata: 2.,
        maneuver_rating: 0.,
        maneuvering: false,
        speed_kts: 3438.,
        signature: 2,
        combat_generation: 4,
        automatic: false,
        target_is_missile: true,
        range_nm: 10.,
        bearing: a::Bearing::Closing,
        terminal_maneuvers: false,
        sea_skimming: false,
        sea_capability: 2,
        noe: false,
        manpads_or_salh: false,
        high_diving: false,
        land_clutter: false,
        radar_guided: true,
        countermeasures: None,
        hoj: false,
    };
    let Ok(budget) = a::intercept_budget(4, false, 16000., 3438., 68.8, 1650., 3.5, false, None)
    else {
        return Err("intercept budget rejected");
    };
    let permit = a::Permit {
        key: "scenario_assumption: frozen defense source packet".into(),
        channel: 0,
        shooter: d,
        target: m,
        weapon: "cn_hhq9".into(),
        tick: w.tick,
        band: a::Band::Long1,
        budget,
        initial_course: 0.,
        initial_range: 10.,
        attack,
        missiles: 1,
    };
    if a::commit(w.rules.aaw.as_mut().unwrap(), permit, &[q]).is_err() {
        return Err("defense packet commit rejected");
    }
    for _ in 0..MAX_ADVANCES {
        harpoon_core::weapons::advance_projectiles(w);
        if w.rules.aaw.as_ref().unwrap().outcomes.values().next().is_some() {
            return Ok(());
        }
    }
    Err("no aaw outcome within the real-flight advance window")
}

#[test]
fn armed_nuclear_missile_really_destroyed_by_defense_must_roll_the_printed_50_percent() {
    for seed in 4_800_001u64..(4_800_001 + MAX_SEEDS as u64) {
        let (mut w, b, d) = fixture_armed(seed);
        let essm = w.db.weapons["us_essm_i"].clone();
        // Incoming armed fuzed nuclear missile: free_fall arms at the first real
        // in-flight observation, exactly as the printed free-fall rule states.
        let Some(m) = harpoon_core::weapons::launch(&mut w, b, &essm, Some(d), None) else {
            continue;
        };
        harpoon_core::weapons::advance_projectiles(&mut w); // real observation arms the warhead
        if !w.entity(m).is_some_and(|e| e.alive) {
            continue;
        }
        if run_defense_to_outcome(&mut w, d, m).is_err() {
            continue;
        }
        let outcome = w.rules.aaw.as_ref().unwrap().outcomes.values().next().unwrap();
        if !outcome.hit {
            continue;
        }
        // Fixture preconditions satisfied: a real non-nuclear defense packet really
        // destroyed the armed nuclear carrier in real flight. The body must be retired.
        assert!(
            !w.entity(m).unwrap().alive,
            "a real intercept hit must retire the incoming carrier body"
        );
        // SOURCE EXPECTATION (red today): the destruction of a fuzed armed nuclear
        // weapon rolls the printed D100. Current production retires it with
        // Removal::Unresolved, so this trace does not exist yet.
        assert!(
            carrier_destroyed_trace_present(&w),
            "14.1.8 Fuzing: real destruction of a fuzed armed nuclear carrier must emit \
             '14.1.8.carrier_destroyed' (D100 vs 0.5); today weapons.rs::source_air_terminal \
             -> remove_projectile(Removal::Unresolved) skips the roll — the sink_by \
             CarrierDestroyed hook (damage.rs) is unreachable from the real defense kill"
        );
        // Reproduce the die identity exactly as nuclear2026::removed derives it, and
        // witness both printed branches of the 50% roll.
        let d100 = 1u8
            + (harpoon_core::Rng::new(seed)
                .derive(harpoon_core::Rng::hash_tag(
                    0,
                    0,
                    &format!("14.1.8:{}:carrier_destroyed", m.0),
                ))
                .next_f64()
                * 100.)
                .floor() as u8;
        let burst_id = format!("14.1.8:{}", m.0);
        n::step(&mut w);
        if d100 <= 50 {
            let causes = detonation_causes(&w);
            assert_eq!(
                causes.len(),
                1,
                "a surviving 50% roll must queue exactly one burst"
            );
            assert_eq!(
                causes[0],
                "CarrierDestroyed",
                "the witnessed burst must carry the carrier-destruction cause (detail.burst.cause)"
            );
            let state = serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
            assert!(
                state["bursts"].get(&burst_id).is_some(),
                "burst {burst_id} must be persisted for replay"
            );
        } else {
            assert!(
                detonation_causes(&w).is_empty(),
                "a failed 50% roll must not detonate"
            );
        }
        // Whole resolved state survives an exact save/load roundtrip.
        let saved = serde_json::to_vec(&w).unwrap();
        let restored: World = serde_json::from_slice(&saved).unwrap();
        assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
        return;
    }
    panic!("no seed in 8 attempts produced a real defense intercept of the armed nuclear carrier; report as fixture precondition, not as the source mechanism");
}

#[test]
fn unarmed_carrier_really_destroyed_by_defense_must_stay_silent() {
    for seed in 4_900_001u64..(4_900_001 + MAX_SEEDS as u64) {
        let (mut w, b, _aim, shooter) = fixture_unarmed(seed);
        let essm = w.db.weapons["us_essm_i"].clone();
        // Aim point 45nm away: at 2250kts the printed two-Increment (60s) arming window
        // has not elapsed when a nearby defender kills the missile seconds into flight.
        let Some(m) = harpoon_core::weapons::launch(&mut w, b, &essm, Some(_aim), None) else {
            continue;
        };
        harpoon_core::weapons::advance_projectiles(&mut w);
        if !w.entity(m).is_some_and(|e| e.alive) {
            continue;
        }
        if run_defense_to_outcome(&mut w, shooter, m).is_err() {
            continue;
        }
        let outcome = w.rules.aaw.as_ref().unwrap().outcomes.values().next().unwrap();
        if !outcome.hit {
            continue;
        }
        assert!(
            !w.entity(m).unwrap().alive,
            "the real intercept must still retire the carrier body"
        );
        // SOURCE EXPECTATION (green boundary, holds before and after the Gap-A fix):
        // only a FUZED warhead rolls on carrier destruction. A not-yet-fuzed delivery
        // destroyed in flight produces no roll, no burst, no detonation, no pending.
        assert!(
            !carrier_destroyed_trace_present(&w),
            "an unarmed (not yet fuzed) carrier destruction must not roll"
        );
        assert!(detonation_causes(&w).is_empty(), "no detonation without a fuzed roll");
        let state = serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
        assert!(
            state["bursts"].as_object().map_or(true, |o| o.is_empty()),
            "no burst may be queued for an unarmed carrier kill"
        );
        let saved = serde_json::to_vec(&w).unwrap();
        let restored: World = serde_json::from_slice(&saved).unwrap();
        assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
        return;
    }
    panic!("no seed in 8 attempts produced a real defense intercept of the unarmed carrier; report as fixture precondition");
}

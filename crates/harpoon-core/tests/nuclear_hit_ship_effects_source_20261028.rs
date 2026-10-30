//! ZCODE-NUCLEAR-NEXT-02 candidate. Suggested target: crates/harpoon-core/tests/.
//! Unregistered artifact; root owns registration and the normal-compile run.
//!
//! Source (English2026 PDF131/printed 14-5 and PDF132/printed 14-6, visually reviewed
//! 2026-10-28): the ship-kill radius destroys any surface vessel; the damage ring takes
//! D6 x 10% of original damage points; a surface burst's effectiveness is halved against
//! a submerged target; one D10 function roll per detonation; blueout per §5.4.6.5.
//!
//! Gap closed by this candidate: no test anywhere drives the successful-function ship
//! chain through the real hit producer damage.rs::apply_projectile_hit ->
//! nuclear2026::terminal_with_target (the conventional-damage suppression hook), so the
//! kill-radius destruction, the D6x10% damage-ring DP, the typed
//! PrimaryDamageEvidence -> signature_damage2026::commit_nuclear provenance and the
//! opposite-medium halving pending band are all implemented but never witnessed.
//! Green-expected: it asserts current correct behaviour with precise, source-derived
//! values; any first failure is a fixture/seed predicate issue to report, not license
//! to change production. Laboratory fit only (scenario_assumption), 1 kt exact row.
use harpoon_core::rules::nuclear2026 as n;
use harpoon_core::{Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::json;

fn fixture(seed: u64) -> (World, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"nuclear-hit-ship-effects-source",
        "name":"scenario_assumption: nuclear surface-hit effects laboratory",
        "duration_s":10000,"seed":seed,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "nuclear_2026":{"source":"scenario_assumption: laboratory delivery, not a historical nuclear ESSM variant",
                "units":{"LAB":{"us_essm_i":{"source":"scenario_assumption: laboratory-only delivery","source_variant":"scenario_assumption:hit_lab","allowed_yields_kt":[1],"selected_yield_kt":1,"medium":"surface","fuze":"free_fall","authorized":true}}}}},
        "units":[
            {"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_052d","name":"TARGET","pos":[0,0.6],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_052d","name":"SHIP2","pos":[0.3,0.6],"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":"us_virginia","name":"BYST","pos":[0.15,0.6],"speed_kts":0,"depth_m":100,"fire_on":false}
        ]})).unwrap();
    let w = harpoon_core::load_scenario(sc, db).unwrap();
    let b = w.entities.values().find(|e| e.name == "LAB").unwrap().id;
    (w, b)
}

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}

fn traces<'a>(w: &'a World, rule: &str) -> Vec<serde_json::Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule: r, detail, .. } if r == rule => {
                Some(serde_json::from_str::<serde_json::Value>(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn actual_hit_with_successful_function_applies_the_printed_ship_effects_and_honest_pending() {
    // 1 kt exact row: ship kill 0.19, aircraft kill 0.29, damage 0.46 (nuclear-2026.json).
    // Geometry: TARGET at GZ (destroyed); SHIP2 at 0.30 (damage ring, D6x10%);
    // BYST submerged at 0.15 (halved kill 0.095 < 0.15 <= halved damage 0.23 -> pending);
    // LAB at 0.60 (outside every ring).
    // Root review convention: at most 8 seed attempts per candidate run.
    for seed in [
        77_000_001u64, 77_000_002, 77_000_003, 77_000_004,
        77_000_005, 77_000_006, 77_000_007, 77_000_008,
    ] {
        let (mut w, b) = fixture(seed);
        let target = id(&w, "TARGET");
        let ship2 = id(&w, "SHIP2");
        let byst = id(&w, "BYST");
        let essm = w.db.weapons["us_essm_i"].clone();
        let Some(p) = harpoon_core::weapons::launch(&mut w, b, &essm, Some(target), None) else {
            continue;
        };
        // Reproduce the function die identity exactly as nuclear2026::step derives it and
        // require a successful (non-dud) detonation for this positive-effects witness.
        let d10 = 1u8
            + (harpoon_core::Rng::new(seed)
                .derive(harpoon_core::Rng::hash_tag(0, 0, &format!("14.1.8:{}:function", p.0)))
                .next_f64()
                * 10.)
                .floor() as u8;
        if d10 == 10 {
            continue;
        }
        let burst_id = format!("14.1.8:{}", p.0);
        let original_hp = w.entity(ship2).unwrap().hp;
        // Real producer hook: the actual-hit path that suppresses conventional damage.
        harpoon_core::damage::apply_projectile_hit(&mut w, p, target, &essm, false, Side::Blue);
        let state = serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
        assert_eq!(
            state["flights"][p.0.to_string()]["terminal"], true,
            "the nuclear flight must be terminal after the actual hit"
        );
        n::step(&mut w);
        // Exactly one function roll for the one burst, and it succeeded by seed predicate.
        let detonations = traces(&w, "14.1.8.detonation");
        assert_eq!(detonations.len(), 1, "one burst, one printed D10");
        assert_eq!(detonations[0]["failed"], false);
        // Ship kill radius: the hit target at GZ is destroyed for game purposes.
        assert!(
            !w.entity(target).unwrap().alive,
            "a vessel inside the ship kill radius is destroyed"
        );
        // Damage ring: D6 x 10% of original DP, with the typed-provenance ship trace.
        let damage_traces = traces(&w, "14.1.8.ship_damage");
        assert_eq!(damage_traces.len(), 1, "exactly the ring ship takes the D6 roll");
        assert_eq!(damage_traces[0]["burst"], burst_id);
        let d6 = damage_traces[0]["d6"].as_u64().unwrap();
        let original_dp = damage_traces[0]["original_dp"].as_f64().unwrap();
        let damage = damage_traces[0]["damage"].as_f64().unwrap();
        assert!((original_dp - original_hp).abs() < 1e-9, "roll is against original DP");
        assert!((damage - original_dp * d6 as f64 * 0.1).abs() < 1e-9, "D6 x 10% of original DP");
        let hp_now = w.entity(ship2).unwrap().hp;
        assert!((hp_now - (original_hp - damage)).abs() < 1e-9, "HP equals original minus committed nuclear DP");
        assert!(
            w.events.iter().any(|e| matches!(
                &e.kind,
                EventKind::Impact { unit, .. } if *unit == ship2
            )),
            "the ring ship's owner receives the Impact report"
        );
        // Typed provenance for the shared signature chain (shared-hooks.md): the
        // systems-damage pending key is recorded while per-component allocation stays null.
        let pending_key = format!("{burst_id}:{}:systems_damage", ship2.0);
        let state = serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
        assert!(
            state["pending"].get(&pending_key).is_some(),
            "note-1 weapons/sensors damage stays an explicit pending, never an invented allocation"
        );
        // Halved effectiveness vs the submerged target: no kill at 0.15nm, and the
        // damage band stays the printed-clause pending instead of invented submarine DP.
        assert!(w.entity(byst).unwrap().alive, "surface burst is halved against a submerged target");
        let pendings = traces(&w, "14.1.8.pending");
        assert!(
            pendings
                .iter()
                .any(|v| v["reason"].as_str().unwrap_or_default().contains("submarine damage-radius")),
            "the submarine damage-radius band must stay pending, not borrow the surface-ship D6 rule"
        );
        // Blueout accompanies every non-air burst (§5.4.6.5 consumer chain).
        let blueouts = traces(&w, "5.4.6.5.blueout");
        assert_eq!(blueouts.len(), 1, "one surface burst, one blueout D6");
        // Deterministic persistence of the whole resolved state.
        let saved = serde_json::to_vec(&w).unwrap();
        let restored: World = serde_json::from_slice(&saved).unwrap();
        assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
        return;
    }
    panic!("no seed in range produced a successful nuclear function; report as fixture precondition");
}

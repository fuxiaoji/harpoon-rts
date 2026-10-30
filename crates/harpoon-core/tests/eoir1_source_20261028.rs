//! ZCODE-EOIR1-01 source candidates for 9.6.8 first-generation EO/IR
//! (English 2026-01-01 PDF108/109, printed 9-7/9-8).
//! Existing-public-API only: ordinary Scenario JSON, public GuidedSurfaceAttack
//! and SetCourse commands, public save/restore. Real US-annex H2-1 AGM-62 Mk1
//! Walleye I row on an explicit author mounting; no private Flight/contact/
//! dice edits, no rerolls. input(None) really omits the smoke screen; a legal
//! turn is a real ordered/rotating course, not an instantaneous heading jump.
//! Root registers this file as crates/harpoon-core/tests/eoir1_source_20261028.rs.
use harpoon_core::rules::{aircombat2026 as ac, firecontrol2026 as fc};
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};

const W: &str = "lab_agm62_walleye1_eoird1";
/// 1st Gen EO/IR Base Lg-Sm cell (aircombat-2026.json "pgm" row 7, col 0);
/// asserted through the production terminal event, never recomputed locally.
const FULL_PH: f64 = 0.75;
/// 6 nmi release at 528 kts: the packet arrives around tick 43.
const ARRIVAL_TICK: i64 = 46;

fn die(seed: u64, actor: u32, at: i64, round: u16) -> u8 {
    let id = format!("9.6.8:{actor}:{at}:1:{round}");
    let mut r = Rng::new(seed).derive(Rng::hash_tag(at, actor, &id));
    1 + (r.next_f64() * 100.) as u8
}

fn placed(seed: u64, actor: u32, at: i64, rounds: u16) -> bool {
    let id = ac::budget::claim_identity(
        ac::budget::AirAttackKind::Pgm,
        EntityId(actor),
        at,
        &json!({"weapon":W,"rounds":rounds,"briefing":"range"}),
    ) + ":rf-placement";
    let mut r = Rng::new(seed).derive(Rng::hash_tag(0, 0, &id));
    i64::from(1 + (r.next_f64() * 10.) as u8)
        <= fc::tables()["placement_quality"]["Good"].as_i64().unwrap()
}

/// Ordinary pre-construction Scenario seed. Decisive window 19..=75:
/// the full 0.75 chance must hit while the smoke-quartered 0.1875 misses,
/// so whichever comparator production uses is observable. Actual preconditions
/// are asserted after the run; nothing is forced at runtime.
fn seed() -> u64 {
    (0..1_000_000)
        .find(|s| placed(*s, 1, 2, 1) && (19..=75).contains(&die(*s, 1, 2, 0)))
        .unwrap()
}

/// `screen` = declared (from_tick, until_tick); None really omits the key.
fn input(screen: Option<(i64, i64)>) -> Value {
    let mut v = json!({"id":"eoir1-source","name":"source AGM62 Walleye I EO(D)/1 lab","seed":seed(),"duration_s":600,
     "weather":{"visibility_nm":40,"jam":[0,0]},
     "context":{"rules_profile":"hv2026-rts-v4","controlled_pgm_los_2026":{"source":"scenario_assumption: experimental clear optical volume, these bounds are author inputs not book constants","minimum_aircraft_altitude_m":500,"maximum_ray_range_nm":40,"no_intervening_terrain":true},"flight_environment":{"wind_from_deg":0,"wind_kts":0},
      "aviation_2026":{"source":"scenario_assumption: author experimental single-Walleye mount on actual F18E, not a claimed Annex loadout; source row 511kg",
       "loads":{"us_f18e":{"payload_kg":511,"tanks":[],"external_weapon_rounds":{(W):1},"source":"scenario_assumption: exact 1x511kg external Walleye I, source weapon row US Aircraft2025 PDF99/H2-1"}},
       "parking":{},"initial_fuel_nm":{"STRIKE":100},
       "surface_aimpoints":{"range":{"source":"scenario_assumption: prebriefed fixed target coordinates","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
      "detection_2026":{"bindings":{},"targets":{},
       "environment":{"source":"scenario_assumption: controlled clear LOS/no moisture; launching aircraft keeps clear LOS to the fixed target launch-to-impact","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
      "eoir_2026":{"source":"English2026 PDF108/109 printed 9-7/9-8 first-generation EO/IR bullet plus US Aircraft2025 PDF99/H2-1 exact Walleye I; native path prospective, not a SATNAV/SALH alias",
       "weapons":{(W):{"source":"US Aircraft2025 PDF99/H2-1 exact AGM-62 Mk1 Walleye I","guidance":"EO (D)/1","generation":1,"min_range_nm":1.0,"range_nm":16.0,"speed_kts":528,"flight_path":"Ext. Glide","hang_weight_kg":511,"damage_points":47,"penetration_cm":8,"signature":"VSmall"}},
       "fits":{"STRIKE":{"source":"scenario_assumption: author mounting 1x Walleye on real F18E","weapon":W,"rounds":1,"external":true}},
       "briefings":{"range":{"source":"scenario_assumption: allocated fixed target briefing; aspect-independent control, no SATNAV bombsight/altitude gate applies","target":"TARGET","aimpoint":"range","known_kind":"Base","known_motion":"stationary","allowed_sides":["Blue"],
        "fcs":{"source":"scenario_assumption: controlled Good RF quality; coordinates alone never create it","quality":"Good"}}}},
      "ground_2026":{"source":"English2026 source controlled MediumHangar D/100DP/unarmored","facilities":{"TARGET":{"template":"hangar_medium","custom":null,"source":"English2026 PDF118/11-3"}}}},
     "units":[{"side":"Blue","class":"us_f18e","name":"STRIKE","pos":[0,-6],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
      {"side":"Red","class":"airfield","name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}]});
    if let Some((from, until)) = screen {
        v["context"]["eoir_2026"]["smoke_screens"] = json!({"screen1":{
         "source":"scenario_assumption: author declared target smoke screen; 9.6.8 prints no smoke radius/TTL/height, these are declared experiment geometry",
         "target":"TARGET","radius_nm":0.5,"height_m":100,
         "obscures_bomb_sensor_from_tick":from,"until_tick":until}});
    }
    v
}

fn world(screen: Option<(i64, i64)>) -> World {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let def: harpoon_core::db::WeaponDef = serde_json::from_value(json!({
        "id":W,"name":"AGM-62 Walleye I Mk1 source EO(D)/1 laboratory mounting",
        "kind":"Bomb","targets":{"surface":true,"land":true},
        "range_nm":16.0,"min_range_nm":1.0,"speed_kts":528,"pk":0,"salvo":1,"reload_s":0,"damage":47,
        "guidance":"BallisticGun",
        "rule_data":{"source":"US Aircraft2025 PDF99/H2-1 exact Walleye I 1-16nm/528kt/Ext.Glide/511kg/47DP/8cm/VSm; pk0 withdraws legacy fallback, native first-generation EO/IR row required","ship_dp":47,"penetration_cm":8,"profile":"glide","bomb_kind":"LDGP"}
    })).unwrap();
    db.weapons.insert(W.into(), def);
    for class in ["us_f18e", "airfield"] {
        let p = db.platforms.get_mut(class).unwrap();
        p.sensors.clear();
        p.weapons.clear();
    }
    db.platforms
        .get_mut("us_f18e")
        .unwrap()
        .weapons
        .push(harpoon_core::db::WeaponFit {
            weapon: W.into(),
            mounts: 1,
            ammo: 1,
        });
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input(screen)).unwrap(), db)
        .unwrap()
}

fn command(actor: EntityId, rounds: u16) -> (Side, Command) {
    (
        Side::Blue,
        Command::AirCombat {
            unit: actor,
            action: ac::Action::GuidedSurfaceAttack {
                weapon: W.into(),
                rounds,
                briefing: "range".into(),
            },
        },
    )
}

fn terminals(w: &World, actor: EntityId) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule,
                unit: Some(id),
                detail,
                ..
            } if rule == "9.6.8.source_terminal" && *id == actor => {
                let d: Value = serde_json::from_str(detail).unwrap();
                (d["weapon"] == W).then_some(d)
            }
            _ => None,
        })
        .collect()
}

fn released(w: &World) -> bool {
    w.entities
        .values()
        .any(|e| e.projectile.as_ref().is_some_and(|p| p.weapon == W))
}

/// Candidate 1 (book facts PDF109 bullet sentences 2-3): after the actual
/// release the aircraft performs an actual legal turn through the public
/// navigation command; the radio data link is not aspect-dependent, so the
/// chain must keep the legal link and settle the original terminal die at the
/// un-quartered first-generation EO/IR table cell. No SATNAV gate fires and
/// no SALH designator exists in this scenario at all.
#[test]
fn eoir01_post_release_actual_turn_keeps_link_and_full_ph() {
    let mut w = world(None);
    harpoon_core::sim::tick(&mut w, &[]);
    let c = command(EntityId(1), 1);
    harpoon_core::sim::tick(&mut w, &[c]);
    assert!(
        released(&w),
        "the real EO/IR source registry/admission must release a physical packet; the SATNAV-only ready gate (I&Sat/1 row check) cannot satisfy first-generation EO/IR"
    );
    let turn = (
        Side::Blue,
        Command::SetCourse {
            unit: EntityId(1),
            course_deg: 90.,
        },
    );
    harpoon_core::sim::tick(&mut w, &[turn]);
    let a = w.entity(EntityId(1)).unwrap();
    assert!(
        (a.ordered_course_deg - 90.).abs() < 1e-9 && a.course_deg > 0. && a.course_deg < 90.,
        "the releasing aircraft must have an actually accepted turn order in progress after release; a fixture without a real maneuver cannot certify the aspect-independent link"
    );
    assert!(
        w.entity(EntityId(1)).unwrap().alive,
        "the firing plane must stay aloft to maintain the required clear LOS to the target until impact"
    );
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    while w.tick < ARRIVAL_TICK {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    assert!(
        w.entity(EntityId(1)).unwrap().alive,
        "LOS-until-impact precondition must hold at terminal time, not only at launch"
    );
    let t = terminals(&w, EntityId(1));
    assert_eq!(
        t.len(),
        1,
        "exactly one terminal adjudication for the single source packet; no reroll"
    );
    assert!(
        (t[0]["probability"].as_f64().unwrap() - FULL_PH).abs() < 1e-9,
        "the post-release turn must keep the un-quartered 1st Gen EO/IR cell (Base Lg-Sm); an aspect model or a generic LOS quarter here would contradict PDF109 sentence 3"
    );
    assert_eq!(
        t[0]["hit"], true,
        "seed predicate asserted 19<=d100<=75, so the full chance must hit"
    );
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap(),
        "mid-flight save/restore must reproduce the identical World while the packet is in flight"
    );
}

/// Candidate 2 (book fact PDF109 bullet sentence 4): a declared, source-qualified
/// smoke screen obscures the target from the bomb's sensor before arrival; the
/// ORIGINAL same terminal die (identical deterministic identity hash, no
/// reroll) must then settle at the quartered chance 0.75/4. The only difference
/// from the smoke-free baseline run is the declared smoke screen; neither run
/// turns the aircraft, so the aspect/link mechanic is not a confounder here.
#[test]
fn eoir02_declared_smoke_on_bomb_sensor_settles_same_die_at_quarter_ph() {
    let mut base = world(None);
    let mut smoke = world(Some((10, 200)));
    harpoon_core::sim::tick(&mut base, &[]);
    harpoon_core::sim::tick(&mut smoke, &[]);
    let c = command(EntityId(1), 1);
    harpoon_core::sim::tick(&mut base, &[c.clone()]);
    harpoon_core::sim::tick(&mut smoke, &[c]);
    assert!(
        released(&base) && released(&smoke),
        "both runs must release the same physical packet; declared smoke qualifies an actual weapon's sensor view, not a hypothetical attack"
    );
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&smoke).unwrap()).unwrap();
    while base.tick < ARRIVAL_TICK {
        harpoon_core::sim::tick(&mut base, &[]);
        harpoon_core::sim::tick(&mut smoke, &[]);
        harpoon_core::sim::tick(&mut saved, &[]);
    }
    let tb = terminals(&base, EntityId(1));
    let ts = terminals(&smoke, EntityId(1));
    assert_eq!(tb.len(), 1);
    assert_eq!(
        ts.len(),
        1,
        "exactly one terminal adjudication under smoke; the quarter must settle the original die, not roll a new one"
    );
    assert_eq!(
        ts[0]["d100"], tb[0]["d100"],
        "the smoke run must consume the same original terminal die as the smoke-free run"
    );
    assert_eq!(
        ts[0]["d100"].as_u64().unwrap(),
        u64::from(die(seed(), 1, 2, 0)),
        "the terminal die must be the scenario-seed original identity hash"
    );
    assert!(
        (tb[0]["probability"].as_f64().unwrap() - FULL_PH).abs() < 1e-9,
        "baseline without smoke keeps the un-quartered cell"
    );
    assert!(
        (ts[0]["probability"].as_f64().unwrap() - FULL_PH / 4.).abs() < 1e-9,
        "declared smoke obscuring the bomb sensor must quarter the hit chance (divide by four), PDF109 bullet sentence 4"
    );
    assert_eq!(tb[0]["hit"], true);
    assert_eq!(
        ts[0]["hit"], false,
        "seed predicate asserted d100>18 for the quartered chance, so the same die must miss under smoke"
    );
    assert_eq!(
        serde_json::to_value(&smoke).unwrap(),
        serde_json::to_value(&saved).unwrap(),
        "mid-flight save/restore must reproduce the identical smoke-case World"
    );
}

/// Candidate 3 (stickiness): the declared screen expires at tick 20, long
/// before the ~tick-43 arrival. The bomb's sensor view was actually obscured
/// while the screen was up, and 9.6.8 treats that LOS as broken, so the same
/// original die must still settle at the quartered chance. A smoke quarter
/// that evaporates at screen expiry would contradict the printed sentence.
#[test]
fn eoir03_smoke_that_expires_before_arrival_still_quarters_original_die() {
    let mut base = world(None);
    let mut short = world(Some((10, 20)));
    harpoon_core::sim::tick(&mut base, &[]);
    harpoon_core::sim::tick(&mut short, &[]);
    let c = command(EntityId(1), 1);
    harpoon_core::sim::tick(&mut base, &[c.clone()]);
    harpoon_core::sim::tick(&mut short, &[c]);
    assert!(
        released(&base) && released(&short),
        "the expiring-screen case needs the same actual release"
    );
    while base.tick < ARRIVAL_TICK {
        harpoon_core::sim::tick(&mut base, &[]);
        harpoon_core::sim::tick(&mut short, &[]);
    }
    let tb = terminals(&base, EntityId(1));
    let ts = terminals(&short, EntityId(1));
    assert_eq!(tb.len(), 1);
    assert_eq!(
        ts.len(),
        1,
        "exactly one terminal adjudication; the expired screen must not trigger a new roll"
    );
    assert_eq!(
        ts[0]["d100"], tb[0]["d100"],
        "the same original terminal die is consumed in both runs"
    );
    assert!(
        (ts[0]["probability"].as_f64().unwrap() - FULL_PH / 4.).abs() < 1e-9,
        "smoke that obscured the bomb sensor before expiring must still quarter the original chance"
    );
    assert_eq!(
        ts[0]["hit"], false,
        "seed predicate asserted d100>18, so the sticky quartered chance must miss"
    );
}

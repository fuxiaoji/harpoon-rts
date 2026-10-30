
use harpoon_core::rules::{aircombat2026 as ac, firecontrol2026 as fc};
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};

const W: &str = "lab_agm65d_maverick";
/// 1st Gen EO/IR Base Lg-Sm cell (aircombat-2026.json "pgm" row 7, col 0);
/// asserted through the production terminal event, never recomputed locally.
const FULL_PH: f64 = 0.80;
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
/// the full 0.80 chance must hit while the smoke-quartered 0.1875 misses,
/// so whichever comparator production uses is observable. Actual preconditions
/// are asserted after the run; nothing is forced at runtime.
fn seed() -> u64 {
    (0..1_000_000)
        .find(|s| placed(*s, 1, 2, 1) && (21..=80).contains(&die(*s, 1, 2, 0)))
        .unwrap()
}

/// `screen` = declared (from_tick, until_tick); None really omits the key.
fn input(screen: Option<(i64, i64)>) -> Value {
    let mut v = json!({"id":"eoir1-source","name":"source AGM-65D IIRH/2 lab","seed":seed(),"duration_s":600,
     "weather":{"visibility_nm":40,"jam":[0,0]},
     "context":{"rules_profile":"hv2026-rts-v4","controlled_pgm_los_2026":{"source":"scenario_assumption: experimental clear optical volume, these bounds are author inputs not book constants","minimum_aircraft_altitude_m":500,"maximum_ray_range_nm":40,"no_intervening_terrain":true},"flight_environment":{"wind_from_deg":0,"wind_kts":0},
      "aviation_2026":{"source":"scenario_assumption: author experimental single-Maverick mount on actual F18E, not a claimed Annex loadout; source row 220kg",
       "loads":{"us_f18e":{"payload_kg":220,"tanks":[],"external_weapon_rounds":{(W):1},"source":"scenario_assumption: exact 1x220kg external Maverick, source weapon row US Aircraft2025 PDF99/H2-1"}},
       "parking":{},"initial_fuel_nm":{"STRIKE":100},
       "surface_aimpoints":{"range":{"source":"scenario_assumption: prebriefed fixed target coordinates","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
      "detection_2026":{"bindings":{},"targets":{},
       "environment":{"source":"scenario_assumption: controlled clear LOS/no moisture; launching aircraft keeps clear LOS to the fixed target launch-to-impact","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
      "eoir_2026":{"source":"English2026 PDF108/109 printed 9-7/9-8 EO/IR bullet and second-generation table row plus US Aircraft2025 PDF99/H2-1 exact Maverick; native path prospective, not a SATNAV/SALH alias",
       "weapons":{(W):{"source":"US Aircraft2025 PDF99/H2-1 exact AGM-65D Maverick","guidance":"IIRH/2","generation":2,"min_range_nm":0.5,"range_nm":11.0,"speed_kts":791,"flight_path":"Direct","hang_weight_kg":220,"damage_points":24,"penetration_cm":139,"signature":"VSm"}},
       "fits":{"STRIKE":{"source":"scenario_assumption: author mounting 1x Maverick on real F18E","weapon":W,"rounds":1,"external":true}},
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

#[test]
fn base_extra_critical_pending_retains_original_hit_die_and_stock_across_restore() {
    let mut w=world(None); harpoon_core::sim::tick(&mut w,&[]);
    harpoon_core::sim::tick(&mut w,&[command(EntityId(1),1)]); assert!(released(&w));
    for _ in 0..45 {harpoon_core::sim::tick(&mut w,&[]);}
    assert_eq!(w.rules.pending_adjudications.len(),1);
    let arrival=w.rules.pgm.as_ref().unwrap().eoir.arrivals.values().next().unwrap();
    assert_eq!(arrival.original_d100,die(seed(),1,2,0));
    let ammo=w.entity(EntityId(1)).unwrap().weapons.iter().find(|x|x.weapon==W).unwrap().ammo;
    assert_eq!(ammo,0);
    let mut saved:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..10 {harpoon_core::sim::tick(&mut w,&[]);harpoon_core::sim::tick(&mut saved,&[]);}
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&saved).unwrap());
    assert_eq!(w.rules.pgm.as_ref().unwrap().eoir.arrivals.values().next().unwrap().original_d100,die(seed(),1,2,0));
    assert_eq!(w.entity(EntityId(1)).unwrap().weapons.iter().find(|x|x.weapon==W).unwrap().ammo,ammo);
    assert!(!w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,detail,..} if rule=="damage_roll" && detail.contains("14.1.5.airframe_extra"))));
}

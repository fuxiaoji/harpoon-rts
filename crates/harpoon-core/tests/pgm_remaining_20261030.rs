//! Source Cmd/2 game adjudication with an explicitly fictional carrier fit.
//! Native command/release/flight/LOS/terminal, no injected contact or die.
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use harpoon_core::rules::{aircombat2026 as ac, firecontrol2026 as fc};
use serde_json::{json,Value};
const W:&str="lab_bgm71ab_tow";
fn seed()->u64 {
    (0..100000).find(|s| {
        let identity="9.6.8:1:2:1:0";
        let mut r=Rng::new(*s).derive(Rng::hash_tag(2,1,identity));
        let die=1+(r.next_f64()*100.) as u8;
        let placement=ac::budget::claim_identity(ac::budget::AirAttackKind::Pgm,EntityId(1),2,
            &json!({"weapon":W,"rounds":1,"briefing":"range"}))+":rf-placement";
        let mut p=Rng::new(*s).derive(Rng::hash_tag(0,0,&placement));
        (13..=50).contains(&die) && i64::from(1+(p.next_f64()*10.) as u8)<=fc::tables()["placement_quality"]["Good"].as_i64().unwrap()
    }).unwrap()
}
fn world(smoke:bool)->World {
    let mut input=json!({"id":"command-source","name":"scenario_assumption: Cmd2 experimental carriage","seed":seed(),"duration_s":600,
      "weather":{"visibility_nm":40,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},
       "controlled_pgm_los_2026":{"source":"scenario_assumption: clear local optical volume, authored bounds","minimum_aircraft_altitude_m":500,"maximum_ray_range_nm":40,"no_intervening_terrain":true},
       "aviation_2026":{"source":"scenario_assumption: experimental TOW carriage, no historical F18E claim",
        "loads":{"us_f18e":{"payload_kg":38,"tanks":[],"external_weapon_rounds":{(W):2},"source":"scenario_assumption: two actual source 19kg stores"}},
        "parking":{},"initial_fuel_nm":{"STRIKE":100},
        "surface_aimpoints":{"range":{"source":"scenario_assumption: fixed experimental aimpoint","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
       "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: clear daylight","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
       "command_sarh_2026":{"source":"English2026 PDF108 9.6.8 Cmd/SARH plus US Aircraft Annex H2-3",
        "weapons":{(W):{"source":"US Aircraft PDF101 H2-3 BGM-71A/B","guidance":"Cmd/2","generation":2,"min_range_nm":0.1,"range_nm":1.6,"speed_kts":542,"flight_path":"Direct","hang_weight_kg":19,"damage_points":8,"additional_criticals":"","penetration_cm":50,"signature":"VSm"}},
        "fits":{"STRIKE":{"source":"scenario_assumption: two TOW stores on experimental carrier","weapon":W,"rounds":2,"external":true}},
        "briefings":{"range":{"source":"scenario_assumption: explicit allocated fixed point","target":"TARGET","aimpoint":"range","known_kind":"Base","known_motion":"stationary","allowed_sides":["Blue"],"fcs":{"source":"scenario_assumption: declared Good experiment","quality":"Good"}}}},
       "ground_2026":{"source":"English2026 source hangar","facilities":{"TARGET":{"template":"hangar_medium","source":"English2026 PDF118"}}}},
      "units":[{"side":"Blue","class":"us_f18e","name":"STRIKE","pos":[0,-1],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
        {"side":"Red","class":"airfield","name":"TARGET","pos":[0,0],"fire_on":false}]});
    if smoke {input["context"]["command_sarh_2026"]["smoke_screens"]=json!({"screen":{"source":"scenario_assumption: temporary controller-ray cylinder, dimensions not printed","target":"TARGET","radius_nm":0.5,"height_m":100,"obscures_controller_los_from_tick":3,"until_tick":5}});}
    let mut db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for class in ["us_f18e","airfield"] {db.platforms.get_mut(class).unwrap().sensors.clear();}
    db.platforms.get_mut("us_f18e").unwrap().weapons=vec![harpoon_core::db::WeaponFit{weapon:W.into(),mounts:1,ammo:2}];
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(),db).unwrap()
}
fn release(w:&mut World) {
    harpoon_core::sim::tick(w,&[]);
    harpoon_core::sim::tick(w,&[(Side::Blue,Command::AirCombat{unit:EntityId(1),action:ac::Action::GuidedSurfaceAttack{weapon:W.into(),rounds:1,briefing:"range".into()}})]);
    assert!(w.entities.values().any(|e|e.projectile.as_ref().is_some_and(|p|p.weapon==W)),"actual source release required");
}
fn terminal(w:&World)->Value {
    let rows:Vec<Value>=w.events.iter().filter_map(|e|match &e.kind {
        EventKind::RulesEvent{rule,detail,..} if rule=="9.6.8.source_terminal"=>{
            let v:Value=serde_json::from_str(detail).unwrap();(v["weapon"]==W).then_some(v)
        },_=>None}).collect();
    assert_eq!(rows.len(),1,"one original terminal settlement");rows[0].clone()
}
#[test]
fn cmd2_clear_controller_los_uses_source_cell_and_separate_extra_criticals() {
    let mut w=world(false);release(&mut w);
    let def=&w.db.weapons[W];
    assert_eq!(def.damage,8.);assert_eq!(def.rule_data.as_ref().unwrap().extra,"");
    assert_eq!(def.kind,harpoon_core::db::WeaponKind::Ascm);
    let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..28 {harpoon_core::sim::tick(&mut w,&[]);harpoon_core::sim::tick(&mut restored,&[]);}
    let t=terminal(&w);assert_eq!(t["probability"],0.5);assert_eq!(t["hit"],true);
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&restored).unwrap());
}
#[test]
fn cmd2_temporary_actual_controller_obscuration_preserves_original_die_and_quarter() {
    let mut clear=world(false);let mut smoke=world(true);release(&mut clear);release(&mut smoke);
    for _ in 0..28 {harpoon_core::sim::tick(&mut clear,&[]);harpoon_core::sim::tick(&mut smoke,&[]);}
    let a=terminal(&clear);let b=terminal(&smoke);
    assert_eq!(a["d100"],b["d100"]);assert_eq!(a["probability"],0.5);assert_eq!(b["probability"],0.125);
    assert_eq!(a["hit"],true);assert_eq!(b["hit"],false);
}

mod eoir2 {
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
       "briefings":{"range":{"source":"scenario_assumption: allocated fixed target briefing; aspect-independent control, no SATNAV bombsight/altitude gate applies","target":"TARGET","aimpoint":"range","known_kind":"Ship","known_motion":"stationary","allowed_sides":["Blue"],
        "fcs":{"source":"scenario_assumption: controlled Good RF quality; coordinates alone never create it","quality":"Good"}}}},
      "ground_2026":{"source":"English2026 source controlled MediumHangar D/100DP/unarmored","facilities":{"TARGET":{"template":"hangar_medium","custom":null,"source":"English2026 PDF118/11-3"}}}},
     "units":[{"side":"Blue","class":"us_f18e","name":"STRIKE","pos":[0,-6],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
      {"side":"Red","class":"cn_052d","name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}]});
    v["context"].as_object_mut().unwrap().remove("ground_2026");
    v["context"]["pgm_2026"]=json!({"source":"scenario_assumption: no PGM ECM","countermeasures":{"TARGET":{"source":"scenario_assumption: no PGM ECM","generation":0}}});
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
    for class in ["us_f18e", "cn_052d"] {
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
    db.platforms.get_mut("cn_052d").unwrap().rule_data=Some(harpoon_core::rules::damage_runtime::PlatformDamage{source:"scenario_assumption: C unarmored test hull".into(),size:"C".into(),role:"surface".into(),belt_cm:0.,deck_cm:0.,..Default::default()});
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
/// un-quartered second-generation EO/IR table cell. No SATNAV gate fires and
/// no SALH designator exists in this scenario at all.
#[test]
fn eoir2_01_post_release_actual_turn_keeps_link_and_full_ph() {
    let mut w = world(None);
    harpoon_core::sim::tick(&mut w, &[]);
    let c = command(EntityId(1), 1);
    harpoon_core::sim::tick(&mut w, &[c]);
    assert!(
        released(&w),
        "the real EO/IR source registry/admission must release a physical packet; the SATNAV-only ready gate (I&Sat/1 row check) cannot satisfy second-generation EO/IR"
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
        "seed predicate asserted 21<=d100<=80, so the full chance must hit"
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
/// reroll) must then settle at the quartered chance 0.80/4. The only difference
/// from the smoke-free baseline run is the declared smoke screen; neither run
/// turns the aircraft, so the aspect/link mechanic is not a confounder here.
#[test]
fn eoir2_02_declared_smoke_on_bomb_sensor_settles_same_die_at_quarter_ph() {
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
        "seed predicate asserted d100>20 for the quartered chance, so the same die must miss under smoke"
    );
    assert_eq!(
        serde_json::to_value(&smoke).unwrap(),
        serde_json::to_value(&saved).unwrap(),
        "mid-flight save/restore must reproduce the identical smoke-case World"
    );
}


}

mod eoir3 {
use harpoon_core::rules::{aircombat2026 as ac, firecontrol2026 as fc};
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};

const W: &str = "lab_agm65f_maverick";
/// 1st Gen EO/IR Base Lg-Sm cell (aircombat-2026.json "pgm" row 7, col 0);
/// asserted through the production terminal event, never recomputed locally.
const FULL_PH: f64 = 0.85;
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
/// the full 0.85 chance must hit while the smoke-quartered 0.1875 misses,
/// so whichever comparator production uses is observable. Actual preconditions
/// are asserted after the run; nothing is forced at runtime.
fn seed() -> u64 {
    (0..1_000_000)
        .find(|s| placed(*s, 1, 2, 1) && (22..=85).contains(&die(*s, 1, 2, 0)))
        .unwrap()
}

/// `screen` = declared (from_tick, until_tick); None really omits the key.
fn input(screen: Option<(i64, i64)>) -> Value {
    let mut v = json!({"id":"eoir1-source","name":"source AGM-65F IIRH/3 lab","seed":seed(),"duration_s":600,
     "weather":{"visibility_nm":40,"jam":[0,0]},
     "context":{"rules_profile":"hv2026-rts-v4","controlled_pgm_los_2026":{"source":"scenario_assumption: experimental clear optical volume, these bounds are author inputs not book constants","minimum_aircraft_altitude_m":500,"maximum_ray_range_nm":40,"no_intervening_terrain":true},"flight_environment":{"wind_from_deg":0,"wind_kts":0},
      "aviation_2026":{"source":"scenario_assumption: author experimental single-Maverick mount on actual F18E, not a claimed Annex loadout; source row 307kg",
       "loads":{"us_f18e":{"payload_kg":307,"tanks":[],"external_weapon_rounds":{(W):1},"source":"scenario_assumption: exact 1x307kg external Maverick, source weapon row US Aircraft2025 PDF99/H2-1"}},
       "parking":{},"initial_fuel_nm":{"STRIKE":100},
       "surface_aimpoints":{"range":{"source":"scenario_assumption: prebriefed fixed target coordinates","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
      "detection_2026":{"bindings":{},"targets":{},
       "environment":{"source":"scenario_assumption: controlled clear LOS/no moisture; launching aircraft keeps clear LOS to the fixed target launch-to-impact","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
      "eoir_2026":{"source":"English2026 PDF108/109 printed 9-7/9-8 EO/IR bullet and third-generation table row plus US Aircraft2025 PDF99/H2-1 exact Maverick; native path prospective, not a SATNAV/SALH alias",
       "weapons":{(W):{"source":"US Aircraft2025 PDF99/H2-1 exact AGM-65F Maverick","guidance":"IIRH/3","generation":3,"min_range_nm":1.5,"range_nm":13.5,"speed_kts":791,"flight_path":"Direct","hang_weight_kg":307,"damage_points":34,"penetration_cm":8,"signature":"VSm"}},
       "fits":{"STRIKE":{"source":"scenario_assumption: author mounting 1x Maverick on real F18E","weapon":W,"rounds":1,"external":true}},
       "briefings":{"range":{"source":"scenario_assumption: allocated fixed target briefing; aspect-independent control, no SATNAV bombsight/altitude gate applies","target":"TARGET","aimpoint":"range","known_kind":"Ship","known_motion":"stationary","allowed_sides":["Blue"],
        "fcs":{"source":"scenario_assumption: controlled Good RF quality; coordinates alone never create it","quality":"Good"}}}},
      "ground_2026":{"source":"English2026 source controlled MediumHangar D/100DP/unarmored","facilities":{"TARGET":{"template":"hangar_medium","custom":null,"source":"English2026 PDF118/11-3"}}}},
     "units":[{"side":"Blue","class":"us_f18e","name":"STRIKE","pos":[0,-6],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
      {"side":"Red","class":"cn_052d","name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}]});
    v["context"].as_object_mut().unwrap().remove("ground_2026");
    v["context"]["pgm_2026"]=json!({"source":"scenario_assumption: no PGM ECM","countermeasures":{"TARGET":{"source":"scenario_assumption: no PGM ECM","generation":0}}});
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
    for class in ["us_f18e", "cn_052d"] {
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
    db.platforms.get_mut("cn_052d").unwrap().rule_data=Some(harpoon_core::rules::damage_runtime::PlatformDamage{source:"scenario_assumption: C unarmored test hull".into(),size:"C".into(),role:"surface".into(),belt_cm:0.,deck_cm:0.,..Default::default()});
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
/// un-quartered third-generation EO/IR table cell. No SATNAV gate fires and
/// no SALH designator exists in this scenario at all.
#[test]
fn eoir3_01_post_release_actual_turn_keeps_link_and_full_ph() {
    let mut w = world(None);
    harpoon_core::sim::tick(&mut w, &[]);
    let c = command(EntityId(1), 1);
    harpoon_core::sim::tick(&mut w, &[c]);
    assert!(
        released(&w),
        "the real EO/IR source registry/admission must release a physical packet; the SATNAV-only ready gate (I&Sat/1 row check) cannot satisfy third-generation EO/IR"
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
        "seed predicate asserted 21<=d100<=80, so the full chance must hit"
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
/// reroll) must then settle at the quartered chance 0.85/4. The only difference
/// from the smoke-free baseline run is the declared smoke screen; neither run
/// turns the aircraft, so the aspect/link mechanic is not a confounder here.
#[test]
fn eoir3_02_declared_smoke_on_bomb_sensor_settles_same_die_at_quarter_ph() {
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
        "seed predicate asserted d100>20 for the quartered chance, so the same die must miss under smoke"
    );
    assert_eq!(
        serde_json::to_value(&smoke).unwrap(),
        serde_json::to_value(&saved).unwrap(),
        "mid-flight save/restore must reproduce the identical smoke-case World"
    );
}


}

mod eoir3_g_g {
use harpoon_core::rules::{aircombat2026 as ac, firecontrol2026 as fc};
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};

const W: &str = "lab_agm65g_maverick";
/// 1st Gen EO/IR Base Lg-Sm cell (aircombat-2026.json "pgm" row 7, col 0);
/// asserted through the production terminal event, never recomputed locally.
const FULL_PH: f64 = 0.85;
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
/// the full 0.85 chance must hit while the smoke-quartered 0.1875 misses,
/// so whichever comparator production uses is observable. Actual preconditions
/// are asserted after the run; nothing is forced at runtime.
fn seed() -> u64 {
    (0..1_000_000)
        .find(|s| placed(*s, 1, 2, 1) && (22..=85).contains(&die(*s, 1, 2, 0)))
        .unwrap()
}

/// `screen` = declared (from_tick, until_tick); None really omits the key.
fn input(screen: Option<(i64, i64)>) -> Value {
    let mut v = json!({"id":"eoir1-source","name":"source AGM-65G IIRH/3 lab","seed":seed(),"duration_s":600,
     "weather":{"visibility_nm":40,"jam":[0,0]},
     "context":{"rules_profile":"hv2026-rts-v4","controlled_pgm_los_2026":{"source":"scenario_assumption: experimental clear optical volume, these bounds are author inputs not book constants","minimum_aircraft_altitude_m":500,"maximum_ray_range_nm":40,"no_intervening_terrain":true},"flight_environment":{"wind_from_deg":0,"wind_kts":0},
      "aviation_2026":{"source":"scenario_assumption: author experimental single-Maverick mount on actual F18E, not a claimed Annex loadout; source row 307kg",
       "loads":{"us_f18e":{"payload_kg":307,"tanks":[],"external_weapon_rounds":{(W):1},"source":"scenario_assumption: exact 1x307kg external Maverick, source weapon row US Aircraft2025 PDF99/H2-1"}},
       "parking":{},"initial_fuel_nm":{"STRIKE":100},
       "surface_aimpoints":{"range":{"source":"scenario_assumption: prebriefed fixed target coordinates","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
      "detection_2026":{"bindings":{},"targets":{},
       "environment":{"source":"scenario_assumption: controlled clear LOS/no moisture; launching aircraft keeps clear LOS to the fixed target launch-to-impact","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
      "eoir_2026":{"source":"English2026 PDF108/109 printed 9-7/9-8 EO/IR bullet and third-generation table row plus US Aircraft2025 PDF99/H2-1 exact Maverick; native path prospective, not a SATNAV/SALH alias",
       "weapons":{(W):{"source":"US Aircraft2025 PDF99/H2-1 exact AGM-65G Maverick","guidance":"IIRH/3","generation":3,"min_range_nm":1.5,"range_nm":13.5,"speed_kts":791,"flight_path":"Direct","hang_weight_kg":307,"damage_points":34,"penetration_cm":8,"signature":"VSm"}},
       "fits":{"STRIKE":{"source":"scenario_assumption: author mounting 1x Maverick on real F18E","weapon":W,"rounds":1,"external":true}},
       "briefings":{"range":{"source":"scenario_assumption: allocated fixed target briefing; aspect-independent control, no SATNAV bombsight/altitude gate applies","target":"TARGET","aimpoint":"range","known_kind":"Ship","known_motion":"stationary","allowed_sides":["Blue"],
        "fcs":{"source":"scenario_assumption: controlled Good RF quality; coordinates alone never create it","quality":"Good"}}}},
      "ground_2026":{"source":"English2026 source controlled MediumHangar D/100DP/unarmored","facilities":{"TARGET":{"template":"hangar_medium","custom":null,"source":"English2026 PDF118/11-3"}}}},
     "units":[{"side":"Blue","class":"us_f18e","name":"STRIKE","pos":[0,-6],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
      {"side":"Red","class":"cn_052d","name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}]});
    v["context"].as_object_mut().unwrap().remove("ground_2026");
    v["context"]["pgm_2026"]=json!({"source":"scenario_assumption: no PGM ECM","countermeasures":{"TARGET":{"source":"scenario_assumption: no PGM ECM","generation":0}}});
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
    for class in ["us_f18e", "cn_052d"] {
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
    db.platforms.get_mut("cn_052d").unwrap().rule_data=Some(harpoon_core::rules::damage_runtime::PlatformDamage{source:"scenario_assumption: C unarmored test hull".into(),size:"C".into(),role:"surface".into(),belt_cm:0.,deck_cm:0.,..Default::default()});
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
/// un-quartered third-generation EO/IR table cell. No SATNAV gate fires and
/// no SALH designator exists in this scenario at all.
#[test]
fn eoir3_g_01_post_release_actual_turn_keeps_link_and_full_ph() {
    let mut w = world(None);
    harpoon_core::sim::tick(&mut w, &[]);
    let c = command(EntityId(1), 1);
    harpoon_core::sim::tick(&mut w, &[c]);
    assert!(
        released(&w),
        "the real EO/IR source registry/admission must release a physical packet; the SATNAV-only ready gate (I&Sat/1 row check) cannot satisfy third-generation EO/IR"
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
        "seed predicate asserted 21<=d100<=80, so the full chance must hit"
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
/// reroll) must then settle at the quartered chance 0.85/4. The only difference
/// from the smoke-free baseline run is the declared smoke screen; neither run
/// turns the aircraft, so the aspect/link mechanic is not a confounder here.
#[test]
fn eoir3_g_02_declared_smoke_on_bomb_sensor_settles_same_die_at_quarter_ph() {
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
        "seed predicate asserted d100>20 for the quartered chance, so the same die must miss under smoke"
    );
    assert_eq!(
        serde_json::to_value(&smoke).unwrap(),
        serde_json::to_value(&saved).unwrap(),
        "mid-flight save/restore must reproduce the identical smoke-case World"
    );
}


}

mod itow {
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use harpoon_core::rules::{aircombat2026 as ac, firecontrol2026 as fc};
use serde_json::{json,Value};
const W:&str="lab_bgm71c_itow";
fn seed()->u64 {
    (0..100000).find(|s| {
        let identity="9.6.8:1:2:1:0";
        let mut r=Rng::new(*s).derive(Rng::hash_tag(2,1,identity));
        let die=1+(r.next_f64()*100.) as u8;
        let placement=ac::budget::claim_identity(ac::budget::AirAttackKind::Pgm,EntityId(1),2,
            &json!({"weapon":W,"rounds":1,"briefing":"range"}))+":rf-placement";
        let mut p=Rng::new(*s).derive(Rng::hash_tag(0,0,&placement));
        (13..=50).contains(&die) && i64::from(1+(p.next_f64()*10.) as u8)<=fc::tables()["placement_quality"]["Good"].as_i64().unwrap()
    }).unwrap()
}
fn world(smoke:bool)->World {
    let mut input=json!({"id":"command-source","name":"scenario_assumption: Cmd2 experimental carriage","seed":seed(),"duration_s":600,
      "weather":{"visibility_nm":40,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},
       "controlled_pgm_los_2026":{"source":"scenario_assumption: clear local optical volume, authored bounds","minimum_aircraft_altitude_m":500,"maximum_ray_range_nm":40,"no_intervening_terrain":true},
       "aviation_2026":{"source":"scenario_assumption: experimental TOW carriage, no historical F18E claim",
        "loads":{"us_f18e":{"payload_kg":38,"tanks":[],"external_weapon_rounds":{(W):2},"source":"scenario_assumption: two actual source 19kg stores"}},
        "parking":{},"initial_fuel_nm":{"STRIKE":100},
        "surface_aimpoints":{"range":{"source":"scenario_assumption: fixed experimental aimpoint","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
       "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: clear daylight","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
       "command_sarh_2026":{"source":"English2026 PDF108 9.6.8 Cmd/SARH plus US Aircraft Annex H2-3",
        "weapons":{(W):{"source":"US Aircraft PDF101 H2-3 BGM-71C","guidance":"Cmd/2","generation":2,"min_range_nm":0.1,"range_nm":1.6,"speed_kts":542,"flight_path":"Direct","hang_weight_kg":19,"damage_points":8,"additional_criticals":"","penetration_cm":69,"signature":"VSm"}},
        "fits":{"STRIKE":{"source":"scenario_assumption: two TOW stores on experimental carrier","weapon":W,"rounds":2,"external":true}},
        "briefings":{"range":{"source":"scenario_assumption: explicit allocated fixed point","target":"TARGET","aimpoint":"range","known_kind":"Base","known_motion":"stationary","allowed_sides":["Blue"],"fcs":{"source":"scenario_assumption: declared Good experiment","quality":"Good"}}}},
       "ground_2026":{"source":"English2026 source hangar","facilities":{"TARGET":{"template":"hangar_medium","source":"English2026 PDF118"}}}},
      "units":[{"side":"Blue","class":"us_f18e","name":"STRIKE","pos":[0,-1],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
        {"side":"Red","class":"airfield","name":"TARGET","pos":[0,0],"fire_on":false}]});
    if smoke {input["context"]["command_sarh_2026"]["smoke_screens"]=json!({"screen":{"source":"scenario_assumption: temporary controller-ray cylinder, dimensions not printed","target":"TARGET","radius_nm":0.5,"height_m":100,"obscures_controller_los_from_tick":3,"until_tick":5}});}
    let mut db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for class in ["us_f18e","airfield"] {db.platforms.get_mut(class).unwrap().sensors.clear();}
    db.platforms.get_mut("us_f18e").unwrap().weapons=vec![harpoon_core::db::WeaponFit{weapon:W.into(),mounts:1,ammo:2}];
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(),db).unwrap()
}
fn release(w:&mut World) {
    harpoon_core::sim::tick(w,&[]);
    harpoon_core::sim::tick(w,&[(Side::Blue,Command::AirCombat{unit:EntityId(1),action:ac::Action::GuidedSurfaceAttack{weapon:W.into(),rounds:1,briefing:"range".into()}})]);
    assert!(w.entities.values().any(|e|e.projectile.as_ref().is_some_and(|p|p.weapon==W)),"actual source release required");
}
fn terminal(w:&World)->Value {
    let rows:Vec<Value>=w.events.iter().filter_map(|e|match &e.kind {
        EventKind::RulesEvent{rule,detail,..} if rule=="9.6.8.source_terminal"=>{
            let v:Value=serde_json::from_str(detail).unwrap();(v["weapon"]==W).then_some(v)
        },_=>None}).collect();
    assert_eq!(rows.len(),1,"one original terminal settlement");rows[0].clone()
}
#[test]
fn itow_clear_controller_los_uses_source_cell_and_separate_extra_criticals() {
    let mut w=world(false);release(&mut w);
    let def=&w.db.weapons[W];
    assert_eq!(def.damage,8.);assert_eq!(def.rule_data.as_ref().unwrap().extra,"");
    assert_eq!(def.kind,harpoon_core::db::WeaponKind::Ascm);
    let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..28 {harpoon_core::sim::tick(&mut w,&[]);harpoon_core::sim::tick(&mut restored,&[]);}
    let t=terminal(&w);assert_eq!(t["probability"],0.5);assert_eq!(t["hit"],true);
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&restored).unwrap());
}
#[test]
fn itow_temporary_actual_controller_obscuration_preserves_original_die_and_quarter() {
    let mut clear=world(false);let mut smoke=world(true);release(&mut clear);release(&mut smoke);
    for _ in 0..28 {harpoon_core::sim::tick(&mut clear,&[]);harpoon_core::sim::tick(&mut smoke,&[]);}
    let a=terminal(&clear);let b=terminal(&smoke);
    assert_eq!(a["d100"],b["d100"]);assert_eq!(a["probability"],0.5);assert_eq!(b["probability"],0.125);
    assert_eq!(a["hit"],true);assert_eq!(b["hit"],false);
}

}

//! Source consumer boundaries following actual integration22 SALH2 green.
//! Case03 schedules an already-adjudicated source sensor casualty at the common
//! damage -> late-ray boundary; it is not a whole weapon/critical-roll test.
//! Case04 exercises the declared RTS partial-TT smoke pending policy, not a
//! newly inferred printed sub-second smoke lifetime. Production stays frozen.
use harpoon_core::rules::{aircombat2026 as ac, firecontrol2026 as fc};
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
const W: &str = "lab_gbu12_salh2";
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
fn seed(smoke: bool) -> u64 {
    (0..1_000_000)
        .find(|s| {
            if smoke {
                placed(*s, 1, 2, 4)
                    && placed(*s, 4, 221, 1)
                    && die(*s, 1, 2, 0) > 70
                    && die(*s, 1, 2, 1) <= 70
                    && (18..=70).contains(&die(*s, 1, 2, 2))
                    && die(*s, 1, 2, 3) <= 17
            } else {
                placed(*s, 1, 2, 1) && (18..=70).contains(&die(*s, 1, 2, 0))
            }
        })
        .unwrap()
}
fn input(smoke: bool) -> Value {
    let mut v = json!({"id":"salh-source","name":"source GBU12 and actual friendly designator lab","seed":seed(smoke),"duration_s":600,
 "weather":{"visibility_nm":40,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},
 "aviation_2026":{"source":"scenario_assumption: author experimental four-GBU12 load on actual F18E, not a claimed Annex loadout; published277kg each",
 "loads":{"us_f18e":{"payload_kg":1108,"tanks":[],"external_weapon_rounds":{(W):4},"source":"scenario_assumption: exact4x277kg experimental GBU12 external load, source weapon row US2025 PDF101/H2-3"}},
 "parking":{},"initial_fuel_nm":{"STRIKE":100,"NEXT":100},"surface_aimpoints":{"range":{"source":"scenario_assumption: prebriefed fixed target coordinates","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}},
 "detection_2026":{"bindings":{},"targets":{},"environment":{"source":"scenario_assumption: controlled clear LOS/no moisture; no free FCS from spot","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
 "laser_sensors":{"source":"scenario_assumption: actually installed controlled compatible designator/spot hardware","fits":{"PAINT":{"source":"scenario_assumption: actual installed friendly designator retrofit","device":"designator","installed":true,"range_nm":5,"range_source":"scenario_assumption: explicit controlled designator envelope","bearing_policy":"exact_scene_ray"},"STRIKE":{"source":"scenario_assumption: actual installed spot receiver","device":"lst","installed":true,"spot_range_nm":5,"spot_range_source":"scenario_assumption: controlled spot envelope","bearing_policy":"exact_scene_ray"}},
 "initial_jobs":{"PAINT":{"source":"scenario_assumption: allocated fixed target painting job","enabled":true,"purpose":"designation","known_aimpoint":{"x":0,"y":0},"aimpoint_source":"scenario_assumption: own fixed briefing; no hidden current target tracking"}}}},
 "salh_2026":{"source":"English2026 PDF108/109 plus US Aircraft2025 PDF101/H2-3; native path prospective, not an already-supported SATNAV alias",
 "weapons":{(W):{"source":"US Aircraft2025 PDF101/H2-3 exact GBU12 PavewayII","guidance":"SALH","generation":2,"min_range_nm":0.5,"range_nm":3.5,"speed_kts":528,"flight_path":"Glide","hang_weight_kg":277,"damage_points":35,"penetration_cm":4,"signature":"VSmall"}},
 "fits":{"STRIKE":{"source":"scenario_assumption: author mounting4xGBU12 on realF18E","weapon":W,"rounds":4,"external":true}},
 "briefings":{"range":{"source":"scenario_assumption: allocated fixed target/source-compatible friendly designator","target":"TARGET","aimpoint":"range","known_kind":"Base","known_motion":"stationary","allowed_sides":["Blue"],"delivery":"glide","designator":"PAINT","device":"designator","fcs":{"source":"scenario_assumption: controlled GoodRF quality, not created by laser spot","quality":"Good"}}}},
 "ground_2026":{"source":"English2026 source controlled MediumHangar D/100DP/unarmored","facilities":{"TARGET":{"template":"hangar_medium","custom":null,"source":"English2026 PDF118/11-3"}}}},
 "units":[{"side":"Blue","class":"us_f18e","name":"STRIKE","pos":[0,-2],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false},
 {"side":"Blue","class":"ru_ka27","name":"PAINT","pos":[1,-1],"course_deg":0,"speed_kts":0,"altitude_m":1000,"fire_on":false},
 {"side":"Red","class":"airfield","name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}]});
    if smoke {
        v["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":"us_f18e","name":"NEXT","pos":[0,-2.-460.*220./3600.],"course_deg":0,"speed_kts":460,"altitude_m":10000,"fire_on":false}));
        v["context"]["salh_2026"]["fits"]["NEXT"] =
            v["context"]["salh_2026"]["fits"]["STRIKE"].clone();
        v["context"]["detection_2026"]["laser_sensors"]["fits"]["NEXT"] =
            v["context"]["detection_2026"]["laser_sensors"]["fits"]["STRIKE"].clone();
    }
    v
}
fn world_with_input(v: Value) -> World {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let def:harpoon_core::db::WeaponDef=serde_json::from_value(json!({"id":W,"name":"GBU12 PavewayII source SALH2 laboratory mounting","kind":"Bomb","targets":{"surface":true,"land":true},"range_nm":3.5,"min_range_nm":0.5,"speed_kts":528,"pk":0,"salvo":1,"reload_s":0,"damage":35,"guidance":"BallisticGun","rule_data":{"source":"US Aircraft2025 PDF101/H2-3 GBU12; pk0 withdraws legacy fallback, native SALH table required","ship_dp":35,"penetration_cm":4,"profile":"glide","bomb_kind":"LDGP"}})).unwrap();
    db.weapons.insert(W.into(), def);
    for class in ["us_f18e", "ru_ka27", "airfield"] {
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
            ammo: 4,
        });
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(v).unwrap(), db).unwrap()
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
fn prerequisite(w: &World) {
    let laser = &w.rules.detection.as_ref().unwrap().laser;
    assert!(
        laser.beams.values().any(|b| b.purpose
            == harpoon_core::rules::detection2026::laser::Purpose::Designation
            && b.emitter == EntityId(2)
            && b.target == Some(EntityId(3))),
        "actual existing physical friendly designator beam must already reach the fixed target"
    );
    assert!(
        !laser.spots.is_empty(),
        "actual first-class tracker must receive that spot, no synthetic illumination flag"
    );
}

fn world(smoke: bool) -> World {
    world_with_input(input(smoke))
}
/// An explicit subsystem-phase fixture. The real command, source release,
/// inventory and flight up to the last physical second use normal sim::tick.
/// Only this final source-adjudicated casualty boundary is driven separately:
/// native packet advance -> source sensor casualty/common finish -> late SALH.
/// No HP, sensor damage flag, guidance, Contact, packet or dice is assigned.
fn arrival_with_adjudicated_sensor_casualty(w: &mut World) {
    use harpoon_core::rules::{
        damage_runtime as dr, detection2026::laser, expanded_damage2026 as ed,
    };
    let target = EntityId(2);
    let previous_detection_at = w.tick;
    assert!(w
        .rules
        .detection
        .as_ref()
        .unwrap()
        .laser
        .beams
        .values()
        .any(|b| b.emitter == target
            && b.target == Some(EntityId(3))
            && b.at == previous_detection_at));
    assert!(ed::sensor_operational(w, target, "laser:designator").unwrap());
    w.tick += 1; // The standard one-second native clock; no packet/target relocation.
    dr::begin(w);
    harpoon_core::movement::update_movement(w);
    harpoon_core::weapons::update_weapons(w);
    assert_eq!(
        w.rules.pgm.as_ref().unwrap().salh.arrivals.len(),
        1,
        "actual native flight must reach its endpoint before this boundary is tested"
    );
    let before = laser::physical_sensor_components(w, target);
    assert_eq!(
        before.len(),
        1,
        "the source casualty has one real installed laser choice"
    );
    let identity = "scenario_assumption:already-adjudicated-sensor-casualty-before-SALH-late-phase";
    ed::source_critical(w, target, identity, "sensor", ed::CriticalContext {
        source: "scenario_assumption: isolated common sensor casualty consumer; no weapon hit or critical-table draw is claimed".into(),
        dp: 0., penetrated: true, underwater: false, airburst: false, weapon: Some(W.into()),
    }).unwrap();
    assert!(!ed::sensor_operational(w, target, "laser:designator").unwrap());
    assert!(
        w.entity(target).unwrap().alive,
        "this boundary disables the actual device without killing its body"
    );
    assert!(w.rules.expanded_damage.as_ref().unwrap().units[&target]
        .disabled
        .contains(&before[0].0));
    assert!(w.rules.detection.as_ref().unwrap().laser.beams.values().any(|b|
        b.emitter == target && b.at == previous_detection_at),
        "the last normal Detection receipt is deliberately still present; it cannot qualify the current late ray");
    dr::finish(w);
    harpoon_core::rules::pgm2026::resolve_deferred_guidance(w);
    let t = terminals(w, EntityId(1));
    assert_eq!(t.len(), 1);
    assert!((t[0]["probability"].as_f64().unwrap() - 0.175).abs() < 1e-9);
    assert_eq!(t[0]["salh"]["continuity_lost"], true);
    assert_eq!(
        t[0]["salh"]["last_actual_sample"]["at"].as_i64(),
        Some(w.tick)
    );
    assert_eq!(t[0]["salh"]["last_actual_sample"]["active"], false);
    let after = serde_json::to_value(&*w).unwrap();
    harpoon_core::rules::pgm2026::resolve_deferred_guidance(w);
    assert_eq!(
        serde_json::to_value(&*w).unwrap(),
        after,
        "the same late phase must not repeat a terminal roll, DP or stock consumption"
    );
}
#[test]
fn salh03_actual_sensor_casualty_precedes_late_arrival_and_old_spot_cannot_supply_clear_los() {
    let mut w = world(false);
    harpoon_core::sim::tick(&mut w, &[]);
    prerequisite(&w);
    harpoon_core::sim::tick(&mut w, &[command(EntityId(1), 1)]);
    let (packet, aim) = w
        .rules
        .pgm
        .as_ref()
        .unwrap()
        .flights
        .iter()
        .find(|(_, f)| f.actor == EntityId(1))
        .map(|(id, f)| (*id, f.aim))
        .unwrap();
    let step = 528. / 3600.;
    while w.entity(packet).unwrap().pos.range_to(aim) > step + 1e-9 {
        assert!(
            w.tick < 40,
            "actual flight must approach its source fixed point"
        );
        harpoon_core::sim::tick(&mut w, &[]);
        assert!(terminals(&w, EntityId(1)).is_empty());
    }
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    arrival_with_adjudicated_sensor_casualty(&mut w);
    arrival_with_adjudicated_sensor_casualty(&mut restored);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
fn overlap_input() -> Value {
    let mut v = input(true);
    let selected = (0..1_000_000u64)
        .find(|s| {
            placed(*s, 1, 2, 4)
                && placed(*s, 4, 177, 1)
                && die(*s, 1, 2, 0) > 70
                && die(*s, 1, 2, 1) <= 70
                && (18..=70).contains(&die(*s, 1, 2, 2))
                && die(*s, 1, 2, 3) <= 17
        })
        .unwrap();
    v["seed"] = json!(selected);
    // Actual flight reaches the release envelope before the next TT, while its
    // bomb arrives after180 but before the previous first-explosion+180 clock.
    v["units"][3]["pos"] = json!([0., -2. - 460. * 176. / 3600.]);
    v
}
fn actor_rounds(w: &World, actor: EntityId) -> (u32, u16) {
    let physical = w
        .entity(actor)
        .unwrap()
        .weapons
        .iter()
        .filter(|m| m.weapon == W)
        .map(|m| u32::from(m.mounts) * u32::from(m.ammo))
        .sum();
    let mass = w.rules.aviation.as_ref().unwrap().flights[&actor].external_weapon_rounds[W];
    (physical, mass)
}
#[test]
fn salh04_previous_smoke_overlapping_new_turn_remains_specific_pending_with_original_evidence() {
    let mut w = world_with_input(overlap_input());
    harpoon_core::sim::tick(&mut w, &[]);
    prerequisite(&w);
    harpoon_core::sim::tick(&mut w, &[command(EntityId(1), 4)]);
    while w.tick < 40 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(terminals(&w, EntityId(1)).len(), 4);
    let smoke = w
        .rules
        .pgm
        .as_ref()
        .unwrap()
        .salh
        .smoke
        .values()
        .next()
        .unwrap()
        .clone();
    assert_eq!(smoke.count, 4);
    let hp = w.entity(EntityId(3)).unwrap().hp;
    while w.tick < 176 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(actor_rounds(&w, EntityId(4)), (4, 4));
    harpoon_core::sim::tick(&mut w, &[command(EntityId(4), 1)]);
    assert_eq!(w.tick, 177);
    assert_eq!(
        actor_rounds(&w, EntityId(4)),
        (3, 3),
        "one real release pays physical and external mass once"
    );
    let packet = *w
        .rules
        .pgm
        .as_ref()
        .unwrap()
        .flights
        .iter()
        .find(|(_, f)| f.actor == EntityId(4))
        .unwrap()
        .0;
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    while w.tick < 200 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    let pgm = w.rules.pgm.as_ref().unwrap();
    let arrival = &pgm.salh.arrivals[&packet];
    assert!(arrival.at.div_euclid(180)>smoke.source_turn && arrival.at<smoke.clear_at,
        "the actual trajectory must enter the precise unprinted partial-TT overlap, not the old220s safe control");
    assert_eq!(
        arrival.ordinal, None,
        "unknown overlapping expiry must not consume a new smoke slot"
    );
    assert_eq!(arrival.original_d100, die(w.seed, 4, 177, 0));
    assert!(w.rules.pending_adjudications.contains_key(&packet));
    assert!(!pgm.outcomes.contains_key(&packet));
    assert!(
        terminals(&w, EntityId(4)).is_empty(),
        "do not guess either clear or quartered Ph at an unprinted RTS boundary"
    );
    assert_eq!(actor_rounds(&w, EntityId(4)), (3, 3));
    assert_eq!(
        w.entity(EntityId(3)).unwrap().hp,
        hp,
        "pending placement adds no guessed target DP"
    );
    assert!(
        w.events
            .iter()
            .any(|e| matches!(&e.kind,EventKind::RulesEvent{rule,detail,..}
        if rule=="9.6.8.terminal_pending" && detail.contains("smoke overlaps a new TacticalTurn")))
    );
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

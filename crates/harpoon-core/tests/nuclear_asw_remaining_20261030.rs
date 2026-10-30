//! ZCODE nuclear-asw remaining items, 2026-10-30-next batch (GLM exclusive).
//! Real source cases for the remaining 8.5.2 / 14.1.8 branches whose production
//! consumers already exist; nothing here re-implements a consumer.
//!
//! Sources (English2026 PDF131/14-5 and PDF132/14-6, visually reviewed in the
//! 2026-10-26/28/29 batches and reused from artifacts .../nuclear/rag-1418-en.json):
//!   - "Subs are affected at the full distance listed in the effects table by
//!     underwater bursts. A surface burst's effectiveness is halved against a
//!     submerged target."
//!   - "If a guided weapon misses its target, it continues in the previous
//!     direction of flight for one increment (30 seconds), then detonates."
//!   - "Once fuzed, nuclear warheads have a 50% chance of detonating even if the
//!     weapon carrying them is destroyed." / D10 function roll, 10 ("0") fails.
//!   - 10kt row: ship 0.41 / inflight 0.62 / damage 0.99 nmi.
//!   - execution policy (parent nuclear-2026.json): "Bystanders use actual scatter
//!     GZ and qualified discrete radii" — never the author aim or a hidden pointer.
//!
//! Consumers verified 2026-10-30 (see artifact ABI.json):
//!   - 8.5.2 bystander ring: nuclear2026/standoff.rs accept_standoff_nuclear +
//!     step() victim loop measuring e.pos.range_to(burst.pos).
//!   - GuidedMiss 30s: nuclear2026::removed(Removal::GuidedMiss) via
//!     surface2026/antiship.rs:941-943.
//!   - LostAcquiredLock: surface2026/antiship.rs:995 target-body-gone branch.
//!   - Air mission abort: already exercised by nuclear_air_abort_asw_source_20261026.rs;
//!     NOT duplicated here.
//!
//! Explicit authorship boundaries (declared, never disguised):
//!   - Nuclear payloads are `scenario_assumption:` declared fictional experiments on
//!     real hulls/weapons; Harpoon V has no implemented nuclear RGM-84D/VLA fit.
//!   - The antiship chain reuses the batch-71 verified Source-Fire environment with
//!     its authored FCS/awareness premises; the ASW chain reuses the wide-batch
//!     laboratory sonar/standoff declarations. All motion runs through real
//!     sim::tick advancement.
//!   - No die, HP or projectile position is written. The antiship Contact and category are explicit pre-adjudicated inputs, not natural reconnaissance. Dice are
//!     prefiltered only through the public Rng API and the real-scenario candidate
//!     loop is capped at 32 seeds per case; every asserted adjudication die is
//!     recomputed and must match the persisted trace.
//!
//! NOTE: compiles once root wires `#[path="nuclear2026/asw_remaining.rs"] pub mod
//! asw_remaining;` into rules/nuclear2026.rs (artifact ABI.json); until then this
//! target is intentionally not part of any selected batch.
use harpoon_core::rules::asw_weapons2026 as a;
use harpoon_core::rules::nuclear2026::asw_remaining;
use harpoon_core::rules::nuclear2026::{self as n, Effect, Medium};
use harpoon_core::rules::surface2026::antiship::Phase;
use harpoon_core::types::PlatformKind;
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, Vec2, World};
use serde_json::{json, Value};

const RING_RADII_NM: [f64; 6] = [0.5, 1.0, 1.5, 2.0, 2.5, 3.0];
const RING_BEARINGS_DEG: [f64; 6] = [60.0, 120.0, 180.0, 240.0, 300.0, 360.0];

fn ring_pos(ri: usize, bi: usize) -> Vec2 {
    let rho = RING_RADII_NM[ri];
    let rad = RING_BEARINGS_DEG[bi].to_radians();
    // Vec2::from_bearing = (sin, cos): bearing 0 points to +y (geom.rs:31).
    Vec2::new(rho * rad.sin(), 12.0 + rho * rad.cos())
}
fn ring_name(ri: usize, bi: usize) -> String {
    format!("RING_{ri}_{bi}")
}
fn ring_units() -> Vec<Value> {
    let mut v = Vec::new();
    for ri in 0..RING_RADII_NM.len() {
        for bi in 0..RING_BEARINGS_DEG.len() {
            let rho = RING_RADII_NM[ri];
            let rad = RING_BEARINGS_DEG[bi].to_radians();
            let (x, y) = (rho * rad.sin(), 12.0 + rho * rad.cos());
            v.push(
                json!({"side":"Blue","class":"cn_kilo","name":ring_name(ri,bi),
                    "pos":[x,y],"speed_kts":0,"depth_m":50,"fire_on":false}),
            );
        }
    }
    v
}

fn detonation_details(w: &World) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "14.1.8.detonation" => {
                Some(serde_json::from_str::<Value>(detail).unwrap())
            }
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
fn fuzed_for(w: &World, projectile: EntityId) -> bool {
    w.events.iter().any(|e| matches!(&e.kind,
        EventKind::RulesEvent { rule, detail, .. } if rule == "14.1.8.fuzed"
            && serde_json::from_str::<Value>(detail).unwrap()["projectile"].as_u64()
                == Some(u64::from(projectile.0))))
}
fn roundtrip(w: &World) {
    let saved = serde_json::to_vec(w).unwrap();
    let restored: World = serde_json::from_slice(&saved).unwrap();
    assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
}

// ---------------------------------------------------------------------------
// Case A environment: the wide-batch verified native ASW standoff chain plus an
// 36-boat bystander ring on the printed 8.5.2 scatter geometry.
// ---------------------------------------------------------------------------
const ACTIVE_SONAR: &str = "sonar_a_asw_lab_active";
const PASSIVE_SONAR: &str = "asw_lab_passive";
const ACTIVE_RANGE_NM: f64 = 16.0;

fn asw_fixture(seed: u64) -> World {
    let launcher = "us_ford";
    let sonar_mount = "bow";
    let context = json!({"rules_profile":"hv2026-rts-v4","nuclear_2026":{"source":"scenario_assumption: explicit nuclear ASW laboratory; not a real VLA warhead","units":{"SENDER":{"us_vlasroc":{"source":"scenario_assumption: declared experimental depth-bomb payload","source_variant":"scenario_assumption: nuclear ASW consumer laboratory","allowed_yields_kt":[20],"selected_yield_kt":20,"medium":"subsurface","fuze":"free_fall","authorized":true}}}},"radar_environment":{"sea_state":0,"precipitation":"None"},
        "asw_weapons_2026":{"source":"scenario_assumption: explicit laboratory carrier/payload fit, Annex E4 transport values","standoffs":{"us_vlasroc":{"source":"scenario_assumption: loaded carrier + Mk54 payload declaration; no real platform fit completeness claim","minimum_range_nm":2.5,"maximum_range_nm":12.0,"carrier_speed_kts":660.0,"guided":true,"payload":{"type":"nuclear","yield_kt":20}}}},
        "torpedo_2026":{"source":"scenario_assumption: explicit payload environment and target signature only","environment":{"source":"scenario_assumption: deep open water","layer_m":null,"reverberant":false},"targets":{"cn_093":{"source":"scenario_assumption: size/noise/coating experiment","size":2,"noise":2,"anechoic":false,"foil_borne":false}}},
        "detection_2026":{"bindings":{
            format!("{launcher}:{ACTIVE_SONAR}"):{"type":"sonar","source":"scenario_assumption: laboratory active Gen5 LMF16nm bow experiment; no real hardware claim","generation":5,"passive_range_nm":null,"active_range_nm":ACTIVE_RANGE_NM,"frequency":"lmf","mount":sonar_mount,"active_torpedo_detection":false,"high_speed_tow":false},
            format!("{launcher}:{PASSIVE_SONAR}"):{"type":"sonar","source":"scenario_assumption: independent laboratory passive Gen5 LMF16nm bow experiment","generation":5,"passive_range_nm":ACTIVE_RANGE_NM,"active_range_nm":null,"frequency":"lmf","mount":sonar_mount,"active_torpedo_detection":false,"high_speed_tow":false}},
            "environment":{"source":"scenario_assumption: open water","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
            "targets":{"cn_093":{"source":"scenario_assumption: controlled target signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}});
    let mut units = vec![
        json!({"side":"Blue","class":launcher,"name":"SENDER","pos":[0,0],"course_deg":0,"speed_kts":0,"depth_m":0,"sonar_active":true,"fire_on":false}),
        json!({"side":"Red","class":"cn_093","name":"TARGET","pos":[0,12],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}),
    ];
    units.extend(ring_units());
    let sc: Scenario = serde_json::from_value(
        json!({"id":"nuclear-asw-remaining","name":"scenario_assumption: 8.5.2 bystander measurement laboratory",
            "seed":seed,"duration_s":5000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":context,"units":units}),
    )
    .unwrap();
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for (sensor_id, kind) in [
        (ACTIVE_SONAR, harpoon_core::db::SensorKind::SonarActive),
        (PASSIVE_SONAR, harpoon_core::db::SensorKind::SonarPassive),
    ] {
        let mut sensor = db.sensors.values().next().unwrap().clone();
        sensor.id = sensor_id.into();
        sensor.name = format!("scenario_assumption: {sensor_id} receiver");
        sensor.kind = kind;
        sensor.air_range_nm = 0.0;
        sensor.surface_range_nm = 0.0;
        sensor.missile_range_nm = 0.0;
        sensor.sub_range_nm = ACTIVE_RANGE_NM;
        db.sensors.insert(sensor.id.clone(), sensor);
    }
    db.platforms.get_mut(launcher).unwrap().sensors =
        vec![ACTIVE_SONAR.into(), PASSIVE_SONAR.into()];
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    db.platforms.get_mut("cn_kilo").unwrap().sensors.clear();
    let mount = db.platforms["us_burke_2a"]
        .weapons
        .iter()
        .find(|m| m.weapon == "us_vlasroc")
        .unwrap()
        .clone();
    db.platforms.get_mut(launcher).unwrap().weapons.push(mount);
    db.validate().unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let actor = id(&w, "SENDER");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSonar {
                unit: actor,
                active: true,
            },
        )],
    );
    while w.tick < 185 {
        assert!(!w.over);
        harpoon_core::sim::tick(&mut w, &[]);
    }
    w
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn rounds(w: &World) -> u16 {
    w.entity(id(w, "SENDER"))
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_vlasroc")
        .unwrap()
        .ammo
}

/// Case A: every ring bystander dies exactly within the printed 20kt submarine
/// radius of the RECORDED actual scatter ground zero, and at least one victim sits
/// farther than that radius from the author aim, proving the measurement basis.
#[test]
fn eight_five_two_bystanders_die_only_within_printed_radius_of_actual_scatter_ground_zero() {
    const SHIP: f64 = 0.51;
    const DAMAGE: f64 = 1.25;
    let mut failures = Vec::new();
    for seed in std::iter::once(20261025u64).chain(5_010_001u64..5_010_001 + 32) {
        let mut w = asw_fixture(seed);
        let shooter = id(&w, "SENDER");
        let target = id(&w, "TARGET");
        let Some(c) = w.sides[Side::Blue.index()]
            .contacts
            .values()
            .find(|c| c.about == target && !c.stale && !c.bearing_only)
            .map(|c| c.id)
        else {
            failures.push(format!("{seed}: no located contact")); continue;
        };
        if harpoon_core::rules::detection2026::known_category(&w, Side::Blue, target) != Some(PlatformKind::Sub) {
            let mut probe = w.rules.detection.as_ref().unwrap().clone();
            let decision = harpoon_core::rules::detection2026::evaluate_world(&w, &mut probe, shooter, PASSIVE_SONAR, target);
            let detection = serde_json::to_value(w.rules.detection.as_ref().unwrap()).unwrap();
            let tracks = detection["tracks"].as_object().map(|t| t.iter().filter(|(k,_)| k.contains(PASSIVE_SONAR) && k.contains(&target.0.to_string())).collect::<Vec<_>>());
            let traces: Vec<_> = w.events.iter().filter_map(|e| match &e.kind { EventKind::RulesEvent { rule, detail, .. } if (rule.contains("sonar") || rule.contains("5.4")) && detail.contains(PASSIVE_SONAR) && detail.contains(&target.0.to_string()) => Some((rule, detail)), _ => None }).rev().take(3).collect();
            failures.push(format!("{seed}: category None decision={decision:?} tracks={tracks:?} traces={traces:?}")); continue; // actual classification
        }
        if let Err(reason) = a::validate_fire(&w, shooter, c, "us_vlasroc") {
            failures.push(format!("{seed}: validation {reason}")); continue;
        }
        // Prefilter the actual public RNG identity for the next physical body.
        // A 9/10 can miss a naturally obtained Fair/Poor solution; no die is injected.
        let projectile = w.next_entity + 1;
        let mut rng = w.rng.derive(harpoon_core::Rng::hash_tag(w.tick + 1, projectile, "8.5.2.placement"));
        let pre_die = 1 + (rng.next_f64() * 10.0).floor() as u8;
        if pre_die < 9 { continue; }
        let stock = rounds(&w);
        harpoon_core::sim::tick(
            &mut w,
            &[(
                Side::Blue,
                Command::Fire {
                    unit: shooter,
                    contact: c,
                    weapon: "us_vlasroc".into(),
                    rounds: 1,
                },
            )],
        );
        assert_eq!(rounds(&w), stock - 1, "real public ASW release must expend one round");
        let mut m: Option<EntityId> = None;
        let mut det: Option<Value> = None;
        for _ in 0..600 {
            if w.over {
                break;
            }
            harpoon_core::sim::tick(&mut w, &[]);
            if m.is_none() {
                if let Some(k) = w
                    .rules
                    .asw_weapons
                    .as_ref()
                    .unwrap()
                    .flights
                    .keys()
                    .next()
                {
                    m = Some(*k);
                }
            }
            if det.is_none() && m.is_some() {
                let key = format!("14.1.8:asw:{}", m.unwrap().0);
                det = detonation_details(&w)
                    .into_iter()
                    .find(|d| d["burst"]["id"].as_str() == Some(key.as_str()));
            }
            if det.is_some() {
                break;
            }
        }
        let Some(m) = m else { failures.push(format!("{seed}: no flight")); continue };
        let Some(det) = det else { failures.push(format!("{seed}: no detonation")); continue };
        let flight = w.rules.asw_weapons.as_ref().unwrap().flights[&m].clone();
        let chance = a::placement_chance(flight.quality, flight.launch_range_nm, true, flight.tdl_modifier, false, Some(20)).unwrap();
        if chance >= 10 { failures.push(format!("{seed}: actual quality {:?} chance {chance}", flight.quality)); continue; }
        let Some(placement) = flight.placement.clone() else { continue };
        assert_eq!(placement.d10, pre_die, "prefilter must match actual persisted die");
        if placement.placed {
            failures.push(format!("{seed}: placed")); continue; // need scatter
        }
        let Some(gz) = flight.payload_position else { continue };
        let state = serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
        let Some(burst) = state["bursts"].get(&format!("14.1.8:asw:{}", m.0)) else {
            continue;
        };
        let effect = burst["primary_asw"]["outcome"]["effect"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if effect != "no_effect" && effect != "pending" {
            failures.push(format!("{seed}: primary effect {effect}")); continue;
        }
        if det["failed"] == json!(true) {
            failures.push(format!("{seed}: function failed")); continue;
        }
        assert_eq!(
            det["burst"]["cause"].as_str(),
            Some("8.5.2 actual carrier placement")
        );
        let aim = flight.aim;
        let mut far_destroyed = 0usize;
        for ri in 0..RING_RADII_NM.len() {
            for bi in 0..RING_BEARINGS_DEG.len() {
                let name = ring_name(ri, bi);
                let b = id(&w, &name);
                let pos = ring_pos(ri, bi);
                let d = pos.range_to(gz);
                let alive = w.entity(b).is_some_and(|e| e.alive);
                assert_eq!(
                    alive,
                    d > SHIP,
                    "bystander {name} at {d:.3}nm from the recorded actual GZ must die exactly \
                     within the printed 0.51nm submarine radius and nowhere else"
                );
                if !alive && pos.range_to(aim) > SHIP {
                    far_destroyed += 1;
                }
                if d > SHIP && d <= DAMAGE {
                    let key = format!("14.1.8:asw:{}:{}", m.0, b.0);
                    assert!(
                        state["pending"].get(&key).is_some(),
                        "submarine detail damage in the 2psi-analogue ring stays pending, never invented"
                    );
                }
            }
        }
        assert!(
            far_destroyed >= 1,
            "a victim whose aim-distance exceeds the printed radius proves measurement from \
             the actual scatter GZ, not from the author aim"
        );
        assert!(
            w.entity(target).is_some_and(|e| e.alive),
            "the original target survives through its 8.5.2 placement outcome, not the blast radius"
        );
        assert!(
            w.events
                .iter()
                .any(|e| matches!(&e.kind, EventKind::RulesEvent { rule, .. } if rule == "5.4.6.5.blueout")),
            "a subsurface burst must apply the printed sonar blueout"
        );
        assert_eq!(detonation_details(&w).len(), 1);
        roundtrip(&w);
        return;
    }
    panic!(
        "no seed in 32 attempts produced a natural 8.5.2 miss with a surviving primary and a \
         functioning detonation; diagnostics: {failures:?}"
    );
}

// ---------------------------------------------------------------------------
// Cases B/C environment: the batch-71 verified antiship Source-Fire chain with a
// declared fictional nuclear payload on the exact IC carrier body.
// ---------------------------------------------------------------------------
const EXACT: &str = "us_rgm84d_ic";
const ACTOR: &str = "scenario_assumption:ic_test_launcher";

fn antiship_fixture(seed: u64) -> (World, EntityId, EntityId, EntityId) {
    antiship_fixture_with_subs(seed, false)
}

fn antiship_fixture_with_subs(seed: u64, subs: bool) -> (World, EntityId, EntityId, EntityId) {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut platform = db.platforms["us_burke_2a"].clone();
    platform.id = ACTOR.into();
    platform.name = "scenario_assumption: isolated IC test launcher".into();
    let mut mount = platform.weapons[0].clone();
    mount.weapon = EXACT.into();
    mount.ammo = 4;
    platform.weapons = vec![mount];
    db.platforms.insert(ACTOR.into(), platform);
    let mut units = vec![
        json!({"side":"Blue","class":ACTOR,"name":"LAUNCHER","pos":[0,0],"speed_kts":0,"fire_on":false}),
        json!({"side":"Red","class":"us_ford","name":"FORD","pos":[0,60],"speed_kts":0,"fire_on":false}),
        json!({"side":"Blue","class":ACTOR,"name":"TRAILER","pos":[0,-2],"speed_kts":0,"fire_on":false}),
        // Author-declared distant inert Red body prevents annihilation ending
        // the scenario before the trailing missile naturally loses its lock.
        json!({"side":"Red","class":"cn_093","name":"DISTANT_RED","pos":[200,200],"speed_kts":0,"depth_m":50,"fire_on":false})];
    if subs {
        for (name, x) in [("SUB_IN", 0.20), ("SUB_OUT", 0.21), ("SUB_DAMAGE_IN", 0.49), ("SUB_DAMAGE_OUT", 0.50)] {
            units.push(json!({"side":"Red","class":"cn_093","name":name,"pos":[x,60],"speed_kts":0,"depth_m":50,"fire_on":false}));
        }
    }
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"nuclear-asw-remaining-ic",
        "name":"scenario_assumption: fictional nuclear IC carrier, real guided-miss and lost-lock branches",
        "duration_s":2000,"seed":seed,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "nuclear_2026":{"source":"scenario_assumption: declared experimental nuclear payload on an exact IC body; not any real RGM-84D variant",
                "units":{"LAUNCHER":{"us_rgm84d_ic":{"source":"scenario_assumption: declared fictional surface-burst experiment on a low-altitude terminally guided carrier","source_variant":"scenario_assumption:gun_ciws_kill_lab","allowed_yields_kt":[10],"selected_yield_kt":10,"medium":"surface","fuze":"terminal_seeker","authorized":true}},"TRAILER":{"us_rgm84d_ic":{"source":"scenario_assumption: fictional second surface payload","source_variant":"scenario_assumption: second test carrier","allowed_yields_kt":[10],"selected_yield_kt":10,"medium":"surface","fuze":"terminal_seeker","authorized":true}}}},
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "radar_land_clutter":[],
            "antiship_2026":{
                "source":"scenario_assumption: declared flight order, observer knowledge and hardkill unavailability; exact weapon/Ford rows remain original source",
                "launchers":[{"actor_name":"LAUNCHER","weapon":EXACT,"exact_variant":"RGM-84D Harpoon IC surface launch","max_rounds_per_turn":4,"source":"scenario_assumption: explicit exact IC carriage on authored trial launcher"},{"actor_name":"TRAILER","weapon":EXACT,"exact_variant":"RGM-84D Harpoon IC surface launch","max_rounds_per_turn":4,"source":"scenario_assumption: second trial launcher"}],
                "author_fcs":[{"actor_name":"LAUNCHER","contact":71,"weapon":EXACT,"quality":"Good","source":"scenario_assumption: controlled own Good solution, not a claimed real FCS device"},{"actor_name":"TRAILER","contact":71,"weapon":EXACT,"quality":"Good","source":"scenario_assumption: pre-adjudicated second solution"}],
                "author_awareness":[{"defender_name":"FORD","shooter_name":"LAUNCHER","weapon":EXACT,"from":0,"until":1800,"source":"scenario_assumption: explicit known incoming attack for controlled countermeasure adjudication"},{"defender_name":"FORD","shooter_name":"TRAILER","weapon":EXACT,"from":0,"until":1800,"source":"scenario_assumption: second incoming attack awareness"}],
                "author_countermeasure_availability":[{"defender_name":"FORD","jamming_available":false,"decoy_available":false,"source":"scenario_assumption: fiction declares no defender J&D fit, so 8.4.1 resolves in base mode only"}]}},
        "units":units
    })).unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let launcher = w
        .entities
        .values()
        .find(|e| e.name == "LAUNCHER")
        .unwrap()
        .id;
    let ford = w.entities.values().find(|e| e.name == "FORD").unwrap().id;
    for unit in w.entities.values_mut() {
        unit.sensors.clear();
        if unit.name != "LAUNCHER" && unit.name != "TRAILER" {
            unit.weapons.clear();
        }
    }
    // Isolate the nuclear question from any conventional IC hit accounting.
    w.db.weapons.get_mut(EXACT).unwrap().pk = 0.;
    w.db.weapons.get_mut(EXACT).unwrap().damage = 0.;
    // Explicit pre-adjudicated located contact and category, matching batch 71;
    // only release, flight, dice and burst are natural, not reconnaissance.
    w.sides[Side::Blue.index()].contacts.insert(ContactId(71),harpoon_core::world::Contact{
        id:ContactId(71),about:ford,category:PlatformKind::Ship,class_name:None,estimate:Vec2::new(0.,60.),
        est_course_deg:180.,est_speed_kts:0.,bearing_only:false,quality:1.,last_update_tick:0,first_tick:-1800,
        stale:false,sensors:vec!["scenario_assumption: pre-adjudicated located attack report; not natural ISR".into()],
        offset:Vec2::zero(),announced:true});
    w.rules.detection.as_mut().unwrap().categories.insert(format!("0:{}",ford.0),PlatformKind::Ship);
    let plot: Command = serde_json::from_value(json!({"cmd":"antiship","unit":launcher,
        "action":{"type":"plot","contact":71,"weapon":EXACT,"course_deg":0.,
            "speed_kts":561.,"enable_run_nm":45.,"waypoints":[],"popup":false}}))
    .unwrap();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, plot)]);
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: launcher,
                contact: ContactId(71),
                weapon: EXACT.into(),
                rounds: 1,
            },
        )],
    );
    assert_eq!(
        w.stats.projectiles_launched, 1,
        "real Source Fire must physically launch the exact IC carrier once"
    );
    let m = *w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .surface
        .antiship
        .flights
        .keys()
        .next()
        .expect("real release must register the complete source flight");
    (w, launcher, ford, m)
}
fn function_die(seed: u64, m: EntityId) -> u8 {
    1 + (harpoon_core::Rng::new(seed)
        .derive(harpoon_core::Rng::hash_tag(
            0,
            0,
            &format!("14.1.8:{}:function", m.0),
        ))
        .next_f64()
        * 10.) as u8
}

/// Case B: a real guided miss retires the carrier, then the fuzed warhead
/// detonates exactly once one printed 30-second continuation forward of the last
/// observed position. No carrier-destruction roll exists on the miss path.
#[test]
fn guided_miss_continues_thirty_seconds_forward_then_detonates_exactly_once() {
    for seed in 5_020_001u64..5_020_001 + 32 {
        let (mut w, launcher, ford, m) = antiship_fixture(seed);
        let mut fuzed = false;
        let mut captured: Option<(Vec2, f64)> = None;
        let mut previous: Option<(Vec2, f64)> = None;
        let mut det: Option<Value> = None;
        for _ in 0..600 {
            if w.over {
                break;
            }
            previous = w.entity(m).map(|e| (e.pos, e.course_deg)).or(previous);
            harpoon_core::sim::tick(&mut w, &[]);
            fuzed |= fuzed_for(&w, m);
            if fuzed && captured.is_none() {
                if let Some(f) = w
                    .rules
                    .aaw
                    .as_ref()
                    .unwrap()
                    .surface
                    .antiship
                    .flights
                    .get(&m)
                {
                    if matches!(f.phase, Phase::Runout) {
                        captured = previous;
                    }
                }
            }
            if captured.is_some() && det.is_none() {
                det = detonation_details(&w)
                    .into_iter()
                    .find(|d| d["burst"]["id"].as_str() == Some(format!("14.1.8:{}", m.0).as_str()));
            }
            if det.is_some() {
                break;
            }
        }
        let Some((pos, course)) = captured else { continue };
        let Some(det) = det else { continue };
        if det["burst"]["cause"].as_str() != Some("GuidedMiss") {
            continue; // seed's lock succeeded; need the printed miss branch
        }
        if det["failed"] == json!(true) {
            continue; // witness a functioning detonation for the geometry check
        }
        assert_eq!(detonation_details(&w).len(), 1, "one miss, one burst");
        assert!(
            !carrier_destroyed_trace_present(&w),
            "the miss path must not roll the carrier-destruction D100"
        );
        let expected = pos.advanced(course, 561.0 * 30.0 / 3600.0);
        let gz: Vec2 = serde_json::from_value(det["burst"]["pos"].clone()).unwrap();
        assert!(
            expected.range_to(gz) <= 0.3,
            "the burst must sit one printed 30-second continuation (~4.675nm) forward of the \
             last observed position, not at the miss point or the target"
        );
        assert!(
            w.entity(m).is_none_or(|e| !e.alive),
            "the real miss retires the carrier body through Runout"
        );
        assert!(matches!(
            w.rules
                .aaw
                .as_ref()
                .unwrap()
                .surface
                .antiship
                .flights[&m]
                .phase,
            Phase::Runout
        ));
        assert!(
            w.entity(ford).unwrap().alive && w.entity(launcher).unwrap().alive,
            "a mid-flight miss detonation tens of nm from both hulls cannot touch them"
        );
        roundtrip(&w);
        return;
    }
    panic!(
        "no seed in 32 attempts produced a natural armed guided miss; report as fixture \
         precondition"
    );
}

/// Case C: fully natural lost-acquired-lock chain. Missile 1 arms, locks, and its
/// functioning armed hit destroys FORD; missile 2 (locked, trailing outside the
/// 0.41nm kill radius of missile 1's ground zero) loses its locked target body and
/// detonates immediately through antiship.rs:995 -> Removal::LostAcquiredLock.
#[test]
fn locked_nuclear_carrier_whose_target_dies_detonates_as_lost_acquired_lock() {
    let mut diagnostics = Vec::new();
    for seed in 5_030_001u64..5_030_001 + 32 {
        let (mut w, launcher, ford, m1) = antiship_fixture(seed);
        if function_die(seed, m1) > 9 {
            continue; // missile 1's armed hit must function to destroy the target
        }
        // A second declared launcher trails by 2 nm; both releases use public
        // Plot/Fire and the same explicit pre-adjudicated contact/FCS premise.
        let trailer = id(&w, "TRAILER");
        let plot: Command = serde_json::from_value(json!({"cmd":"antiship","unit":trailer,
            "action":{"type":"plot","contact":71,"weapon":EXACT,"course_deg":0.,
                "speed_kts":561.,"enable_run_nm":45.,"waypoints":[],"popup":false}})).unwrap();
        harpoon_core::sim::tick(&mut w, &[(Side::Blue, plot)]);
        let before = w.stats.projectiles_launched;
        harpoon_core::sim::tick(&mut w, &[(Side::Blue, Command::Fire {
            unit: trailer, contact: ContactId(71), weapon: EXACT.into(), rounds: 1,
        })]);
        if w.stats.projectiles_launched != before + 1 {
            let events: Vec<_> = w.events.iter().rev().take(8).map(|e| format!("{:?}", e.kind)).collect();
            diagnostics.push(format!("{seed}: second launch blocked count={} events={events:?}", w.stats.projectiles_launched)); continue;
        }
        let Some(m2) = w
            .rules
            .aaw
            .as_ref()
            .unwrap()
            .surface
            .antiship
            .flights
            .keys()
            .copied()
            .find(|k| *k != m1)
        else {
            continue;
        };
        let mut m2_locked = false;
        let mut m2_retired_at: Option<Vec2> = None;
        for _ in 0..800 {
            if w.over {
                break;
            }
            harpoon_core::sim::tick(&mut w, &[]);
            if let Some(f) = w
                .rules
                .aaw
                .as_ref()
                .unwrap()
                .surface
                .antiship
                .flights
                .get(&m2)
            {
                if matches!(f.phase, Phase::LockedApproach) {
                    m2_locked = true;
                }
                if m2_locked && m2_retired_at.is_none() && matches!(f.phase, Phase::Runout) {
                    m2_retired_at = w.entity(m2).map(|e| e.pos);
                }
            }
            if detonation_details(&w).len() >= 2 {
                break;
            }
        }
        let causes: Vec<_> = detonation_details(&w)
            .iter()
            .map(|d| d["burst"]["cause"].as_str().unwrap_or_default().to_string())
            .collect();
        if !m2_locked || causes.len() != 2 {
            let flight = w.rules.aaw.as_ref().unwrap().surface.antiship.flights.get(&m2);
            let nuclear = serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
            let events: Vec<_> = w.events.iter().filter_map(|e| match &e.kind { EventKind::RulesEvent { rule, detail, .. } if rule.contains("8.4") || rule.contains("7.4") || rule.contains("14.1.8") => Some(format!("{rule}:{detail}")), _ => None }).rev().take(8).collect();
            diagnostics.push(format!("{seed}: locked={m2_locked} flight={flight:?} causes={causes:?} nuclear={nuclear} events={events:?}")); continue;
        }
        if !causes.contains(&"Hit".to_string()) || !causes.contains(&"LostAcquiredLock".to_string())
        {
            diagnostics.push(format!("{seed}: unexpected causes {causes:?}")); continue;
        }
        assert!(
            fuzed_for(&w, m2),
            "the lost-lock roll is gated on the printed fuzing: missile 2 must really have armed"
        );
        assert!(
            !carrier_destroyed_trace_present(&w),
            "hit and lost-lock are their own printed causes; no destruction roll may appear"
        );
        assert!(
            !w.entity(ford).unwrap().alive,
            "missile 1's functioning armed hit must destroy the locked target"
        );
        assert!(
            w.entity(m1).is_none_or(|e| !e.alive) && w.entity(m2).is_none_or(|e| !e.alive),
            "both retired carrier bodies must be gone through their own branches"
        );
        let state = serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
        let ford_pos = w.entity(ford).unwrap().pos;
        let g1: Vec2 = serde_json::from_value(state["bursts"][&format!("14.1.8:{}", m1.0)]["pos"].clone()).unwrap();
        let d1 = g1.range_to(ford_pos);
        assert!(
            d1 <= 1e-6,
            "an adjudicated hit detonates at the target ({d1:.6}nm off)"
        );
        let Some(b2) = state["bursts"].get(&format!("14.1.8:{}", m2.0)) else {
            panic!("lost-acquired-lock burst must be persisted for replay")
        };
        if let Some(at) = m2_retired_at {
            let g2: Vec2 = serde_json::from_value(b2["pos"].clone()).unwrap();
            assert!(
                at.range_to(g2) <= 0.3,
                "the lost-lock burst sits at the carrier's last position (immediate, no 30s run)"
            );
        }
        assert!(
            w.entity(launcher).unwrap().alive,
            "the launcher sits 60nm from both ground zeros"
        );
        assert_eq!(detonation_details(&w).len(), 2);
        roundtrip(&w);
        return;
    }
    panic!(
        "no seed produced hit-then-lost-lock; diagnostics: {diagnostics:?}"
    );
}

/// Printed 14.1.8 boundaries through the public game-table API plus the delivered
/// witness contracts. PDF132: submarines take the full distance underwater and half
/// from a surface burst; low-altitude aircraft are destroyed within the ship kill
/// radius by a subsurface burst; submarine detail damage has no printed value.
#[test]
fn printed_sub_distance_boundaries_and_witness_contracts_hold() {
    asw_remaining::cross_check_parent_rules()
        .expect("witness table must not drift from the parent printed values");
    assert!(asw_remaining::policy("bystander_measurement_basis").contains("actual_scatter_ground_zero"));
    assert!(asw_remaining::policy("submarine_detail_damage").starts_with("pending"));
    assert_eq!(n::radii(10).unwrap(), [0.41, 0.62, 0.99]);
    let pending = Effect::Pending(
        "14.1.8 submarine damage-radius casualty not specified by the surface-ship D6 clause",
    );
    assert_eq!(
        n::effect(10, Medium::Subsurface, PlatformKind::Sub, false, false, 0.41).unwrap(),
        Effect::Destroyed
    );
    assert_eq!(
        n::effect(10, Medium::Subsurface, PlatformKind::Sub, false, false, 0.42).unwrap(),
        pending
    );
    assert_eq!(
        n::effect(10, Medium::Surface, PlatformKind::Sub, false, false, 0.205).unwrap(),
        Effect::Destroyed
    );
    assert_eq!(
        n::effect(10, Medium::Surface, PlatformKind::Sub, false, false, 0.21).unwrap(),
        pending
    );
    assert_eq!(
        n::effect(10, Medium::Subsurface, PlatformKind::Aircraft, false, true, 0.41).unwrap(),
        Effect::Destroyed
    );
    assert_eq!(
        n::effect(10, Medium::Subsurface, PlatformKind::Aircraft, false, true, 0.42).unwrap(),
        Effect::None
    );
}

// A fictional surface payload on a real released carrier; contact/FCS are authored
// premises, while flight, lock, function die and burst are actual consumers.
#[test]
fn surface_burst_consumes_half_range_for_submerged_bystanders_at_actual_gz() {
    for seed in 5_040_001u64..5_040_033 {
        let (mut w, _, ford, m) = antiship_fixture_with_subs(seed, true);
        for _ in 0..800 {
            if w.over { break; }
            harpoon_core::sim::tick(&mut w, &[]);
            if !detonation_details(&w).is_empty() { break; }
        }
        let Some(det) = detonation_details(&w).into_iter().find(|d| d["burst"]["cause"] == "Hit" && d["failed"] == false) else { continue };
        let gz: Vec2 = serde_json::from_value(det["burst"]["pos"].clone()).unwrap();
        assert!(gz.range_to(w.entity(ford).unwrap().pos) < 1e-6);
        let state = serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
        for (name, alive, pending) in [("SUB_IN", false, false), ("SUB_OUT", true, true), ("SUB_DAMAGE_IN", true, true), ("SUB_DAMAGE_OUT", true, false)] {
            let sub = id(&w, name);
            assert_eq!(w.entity(sub).unwrap().alive, alive, "{name} distance {} from actual GZ", w.entity(sub).unwrap().pos.range_to(gz));
            let key = format!("14.1.8:{}:{}", m.0, sub.0);
            assert_eq!(state["pending"].get(&key).is_some(), pending, "{name} pending detail damage");
        }
        roundtrip(&w);
        return;
    }
    panic!("no functioning natural hit in 32 seeds; fixture precondition failed");
}

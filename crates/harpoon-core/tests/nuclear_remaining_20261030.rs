//! ZCODE nuclear-20261030 continuation: real source Gun/CIWS destruction of a
//! nuclear-armed carrier, armed positive control and unarmed negative control,
//! plus the printed 14.1.8 submarine-distance public-API case.
//!
//! Source (English2026 PDF132/printed 14-6 §14.1.8 Fuzing, visually reviewed in the
//! 2026-10-28/29 batches and reused here from artifacts .../nuclear/rag-1418-en.json):
//!   "Once fuzed, nuclear warheads have a 50% chance of detonating even if the weapon
//!    carrying them is destroyed."
//!   "If a guided weapon misses its target, it continues in the previous direction of
//!    flight for one increment (30 seconds), then detonates."
//!   "Subs are affected at the full distance listed in the effects table by underwater
//!    bursts. A surface burst's effectiveness is halved against a submerged target."
//! Nuclear Weapons Effects table (PDF131/14-5): 10kt row ship 0.41 / inflight 0.62 /
//! damage 0.99 nmi.
//!
//! Real production path under test (root fix, aaw2026.rs gun branch): the real
//! fire_at_impl Gun/Ciws branch destroys the projectile carrier and calls
//! nuclear2026::removed(Removal::CarrierDestroyed); armed rolls the printed D100,
//! unarmed must stay silent. The old weapons.rs neutral-retirement branch is not
//! touched and the SAM packet path of 20261028 is not copied.
//!
//! Explicit authorship boundaries (declared, never disguised as natural play):
//! - The nuclear payload on the exact us_rgm84d_ic body is a declared fictional
//!   laboratory experiment (`scenario_assumption:` source_variant). Harpoon V has no
//!   implemented nuclear RGM-84D; this is a game-table exercise only.
//! - The LAUNCHER attack uses the verified antiship_2026 Source-Fire chain with its
//!   authored FCS/awareness context (contact 71 premise). The FORD point-defense
//!   track is a single pre-adjudicated own-side contact + known-category premise
//!   (mirroring the accepted aaw_2026.rs controlled-incoming fixture), inserted once
//!   before engagement and never mutated. Natural SPY-3 acquisition is future work.
//! - Apart from the explicit pre-adjudicated attack/defense reports above, no die, HP or projectile position is written by this test.
//!   Every adjudication die is recomputed through the public Rng API and the seed
//!   loop only SELECTS seeds (cap below), exactly like antiship_2026.rs:599 and
//!   nuclear_carrier_destroyed_source_20261028.rs. All motion runs through real
//!   sim::tick flight advancement.
use harpoon_core::rules::aaw2026;
use harpoon_core::rules::nuclear2026 as n;
use harpoon_core::rules::nuclear2026::{Effect, Medium};
use harpoon_core::rules::surface2026::antiship::Phase;
use harpoon_core::types::PlatformKind;
use harpoon_core::Vec2;
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};

const EXACT: &str = "us_rgm84d_ic";
const ACTOR: &str = "scenario_assumption:ic_test_launcher";
const MAX_SEEDS: u64 = 16;
/// PD fire band: far enough above the friendly-restriction increment, close enough
/// that even a one-tick observation lag keeps the burst inside the 0.41nm ship kill
/// radius (0.25 + one 561kt tick of 0.156nm <= 0.41).
const FIRE_RANGE_NM: f64 = 0.25;

fn tag_die(seed: u64, a: i64, b: u32, tag: &str, sides: f64) -> u8 {
    1 + (Rng::new(seed).derive(Rng::hash_tag(a, b, tag)).next_f64() * sides) as u8
}
fn carrier_die(seed: u64, m: EntityId) -> u8 {
    tag_die(
        seed,
        0,
        0,
        &format!("14.1.8:{}:carrier_destroyed", m.0),
        100.,
    )
}
fn function_die(seed: u64, m: EntityId) -> u8 {
    tag_die(seed, 0, 0, &format!("14.1.8:{}:function", m.0), 10.)
}
fn detonation_causes(w: &World) -> Vec<String> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "14.1.8.detonation" => Some(
                serde_json::from_str::<Value>(detail).unwrap()["burst"]["cause"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            ),
            _ => None,
        })
        .collect()
}
fn fuzed_trace_count(w: &World) -> usize {
    w.events
        .iter()
        .filter(|e| {
            matches!(
                &e.kind,
                EventKind::RulesEvent { rule, .. } if rule == "14.1.8.fuzed"
            )
        })
        .count()
}

/// Verified antiship_2026 controlled Source-Fire environment, plus the explicit
/// fictional nuclear payload declared on the exact IC carrier body.
/// `enable_run_nm`: 45.0 arms at 45nm flown (kill happens near 59.6nm); 80.0 never
/// enables across the 60nm flight (unarmed control). Both satisfy the printed
/// validation window [plot_range/2, weapon_range] = [30, 80].
fn fixture(seed: u64, enable_run_nm: f64) -> (World, EntityId, EntityId, EntityId) {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut platform = db.platforms["us_burke_2a"].clone();
    platform.id = ACTOR.into();
    platform.name = "scenario_assumption: isolated IC test launcher".into();
    let mut mount = platform.weapons[0].clone();
    mount.weapon = EXACT.into();
    mount.ammo = 4;
    platform.weapons = vec![mount];
    db.platforms.insert(ACTOR.into(), platform);
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"nuclear-remaining-20261030",
        "name":"scenario_assumption: fictional nuclear payload on an exact IC carrier, destroyed by real source Ford CIWS",
        "duration_s":2000,"seed":seed,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "nuclear_2026":{"source":"scenario_assumption: declared experimental nuclear payload on an exact IC body; not any real RGM-84D variant and not a Harpoon-implemented nuclear weapon",
                "units":{"LAUNCHER":{"us_rgm84d_ic":{"source":"scenario_assumption: declared fictional surface-burst experiment on a low-altitude terminally guided carrier","source_variant":"scenario_assumption:gun_ciws_kill_lab","allowed_yields_kt":[10],"selected_yield_kt":10,"medium":"surface","fuze":"terminal_seeker","authorized":true}}}},
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "radar_land_clutter":[],
            "antiship_2026":{
                "source":"scenario_assumption: declared flight order, observer knowledge and hardkill unavailability; exact weapon/Ford rows remain original source",
                "launchers":[{"actor_name":"LAUNCHER","weapon":EXACT,"exact_variant":"RGM-84D Harpoon IC surface launch","max_rounds_per_turn":4,"source":"scenario_assumption: explicit exact IC carriage on authored trial launcher"}],
                "author_fcs":[{"actor_name":"LAUNCHER","contact":71,"weapon":EXACT,"quality":"Good","source":"scenario_assumption: controlled own Good solution, not a claimed real FCS device"}],
                "author_awareness":[{"defender_name":"FORD","shooter_name":"LAUNCHER","weapon":EXACT,"from":0,"until":1800,"source":"scenario_assumption: explicit known incoming attack for controlled countermeasure adjudication"}],
                "author_countermeasure_availability":[{"defender_name":"FORD","jamming_available":false,"decoy_available":false,"source":"scenario_assumption: fiction declares no defender J&D fit, so 8.4.1 resolves in base mode only"}]}},
        "units":[
            {"side":"Blue","class":ACTOR,"name":"LAUNCHER","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"us_ford","name":"FORD","pos":[0,60],"course_deg":180,"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":"cn_093","name":"scenario_assumption: blue trial reserve","pos":[1000,1000],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"scenario_assumption: red trial reserve","pos":[-1000,-1000],"speed_kts":0,"depth_m":50,"fire_on":false}]
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
        if unit.name != "LAUNCHER" {
            unit.weapons.clear();
        }
    }
    let ship = w.entity_mut(ford).unwrap();
    ship.course_deg = 180.;
    ship.weapons = vec![
        harpoon_core::world::WeaponMount {
            weapon: "ciws_30mm".into(),
            mounts: 3,
            ammo: 21,
            cooldown_s: 0.,
        },
        harpoon_core::world::WeaponMount {
            weapon: "us_essm_i".into(),
            mounts: 1,
            ammo: 16,
            cooldown_s: 0.,
        },
    ];
    ship.sensors = ["radar_air_long", "us_spy3_fc", "visual"]
        .into_iter()
        .map(|sensor| harpoon_core::world::SensorInstance {
            sensor: sensor.into(),
            active: true,
        })
        .collect();
    ship.ext.insert("emission.radar".into(), 1.);
    // Isolate the nuclear question from any conventional IC hit accounting.
    w.db.weapons.get_mut(EXACT).unwrap().pk = 0.;
    w.db.weapons.get_mut(EXACT).unwrap().damage = 0.;
    w.sides[Side::Blue.index()].contacts.insert(ContactId(71),harpoon_core::world::Contact{
        id:ContactId(71),about:ford,category:PlatformKind::Ship,class_name:None,estimate:Vec2::new(0.,60.),
        est_course_deg:180.,est_speed_kts:0.,bearing_only:false,quality:1.,last_update_tick:0,first_tick:-1800,
        stale:false,sensors:vec!["scenario_assumption: pre-adjudicated located attack report; not natural ISR".into()],
        offset:Vec2::zero(),announced:true});
    w.rules.detection.as_mut().unwrap().categories.insert(format!("0:{}",ford.0),PlatformKind::Ship);
    let plot: Command = serde_json::from_value(json!({"cmd":"antiship","unit":launcher,
        "action":{"type":"plot","contact":71,"weapon":EXACT,"course_deg":0.,
            "speed_kts":561.,"enable_run_nm":enable_run_nm,"waypoints":[],"popup":false}}))
    .unwrap();
    plot.validate_source(&w,Side::Blue).expect("declared IC plot preconditions");
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

/// Explicit pre-adjudicated defense premise: one own-side PD track and known
/// category for the real incoming body, inserted once and never mutated.
fn pre_adjudicated_pd_track(w: &mut World, m: EntityId) -> ContactId {
    let cid = ContactId(81);
    let now = w.tick;
    let position = w.entity(m).unwrap().pos;
    w.sides[Side::Red.index()].contacts.insert(
        cid,
        harpoon_core::world::Contact {
            id: cid,
            about: m,
            category: PlatformKind::Missile,
            class_name: None,
            estimate: position,
            est_course_deg: 0.,
            est_speed_kts: 561.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: now,
            first_tick: now,
            stale: false,
            sensors: vec![
                "scenario_assumption: pre-adjudicated own-side point-defense track (frozen defense premise; no natural FCS claim)".into(),
            ],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("{}:{}", Side::Red.index(), m.0),
        PlatformKind::Missile,
    );
    cid
}

/// One real engagement attempt: run the real flight until the CIWS band, fire once
/// through manual_fire only when the precomputed source die hits, and report the
/// real result. `armed_seen` witnesses the printed arming trace in flight.
/// Die selection happens at seed level only: any unfavorable source die abandons
/// the whole seed, and no outcome is ever written by the test.
#[allow(clippy::type_complexity)]
fn engage_once(
    seed: u64,
    enable_run_nm: f64,
    require_armed: bool,
) -> Option<(
    World,
    EntityId,
    EntityId,
    EntityId,
    ContactId,
    String,
    u8,
    u8,
    u8,
)> {
    let (mut w, launcher, ford, m) = fixture(seed, enable_run_nm);
    let carrier = carrier_die(seed, m);
    if require_armed && carrier > 50 {
        // Seed selection only: the armed control must witness the detonating branch.
        return None;
    }
    let function = function_die(seed, m);
    let cid = pre_adjudicated_pd_track(&mut w, m);
    let mut armed_seen = false;
    for _ in 0..600 {
        if w.over {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
        // Observe the actual serialized referee state without setting it;
        // a missing trace must fail its own assertion, not suppress engagement.
        let nuclear_state=serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
        armed_seen |= nuclear_state["flights"][m.0.to_string()]["armed"]==true;
        if require_armed && !armed_seen {
            continue;
        }
        let (mp, mcourse) = {
            let e = w.entity(m).unwrap();
            (e.pos, e.course_deg)
        };
        let fp = w.entity(ford).unwrap().pos;
        let range = fp.range_to(mp);
        if range > FIRE_RANGE_NM {
            continue;
        }
        // Source geometry gates, evaluated on the real world before any fire.
        let ford_course = w.entity(ford).unwrap().course_deg;
        let rel = fp.bearing_to(mp) - ford_course;
        if !aaw2026::arc_allows("SW", rel).unwrap_or(false) {
            return None;
        }
        let aspect = circular_deg(mcourse - mp.bearing_to(fp));
        if aspect > 90. {
            return None;
        }
        let gun = w.db.weapons["ciws_30mm"].clone();
        let Ok(permit) =
            aaw2026::world_gun_solution(&w, w.rules.aaw.as_ref().unwrap(), ford, m, &gun, 0)
        else {
            return None;
        };
        // Exact production identity for the first burst of physical mount 0.
        let key = format!("{}:{}:{}", ford.0, "ciws_30mm", 0usize);
        let identity = format!("aaw:gun:{}:{}:{}:{}", key, w.tick, 1u8, 1u8);
        let d100 = tag_die(seed, w.tick, ford.0, &identity, 100.);
        if d100 as f64 > permit.solution.probability * 100. {
            // Die selection is done at seed level only: this whole seed is abandoned.
            return None;
        }
        assert!(harpoon_core::weapons::manual_fire(
            &mut w,
            Side::Red,
            ford,
            cid,
            "ciws_30mm",
            1
        ));
        return Some((
            w, launcher, ford, m, cid, identity, d100, carrier, function,
        ));
    }
    None
}

fn circular_deg(diff: f64) -> f64 {
    let d = diff.abs() % 360.;
    if d > 180. {
        360. - d
    } else {
        d
    }
}

#[test]
fn armed_nuclear_ic_destroyed_by_real_source_ford_ciws_rolls_the_printed_destruction_d100() {
    for seed in 4_810_001..4_810_001 + MAX_SEEDS {
        let Some((mut w, launcher, ford, m, cid, identity, d100, carrier, function)) =
            engage_once(seed, 45.0, true)
        else {
            continue;
        };
        // The armed flight was really fuzed in flight (printed arming trace).
        assert!(
            fuzed_trace_count(&w) >= 1,
            "enable_run 45nm < kill range must arm the TerminalSeeker fuze via the real flight"
        );
        // Real gun outcome under the exact precomputed identity.
        let outcome = &w.rules.aaw.as_ref().unwrap().outcomes[&identity];
        assert_eq!(outcome.d100, d100);
        assert!(outcome.hit, "the seed was selected for a real source gun hit");
        assert!(
            !w.entity(m).unwrap().alive,
            "a real source CIWS hit must retire the nuclear carrier body"
        );
        assert_eq!(
            w.rules.aaw.as_ref().unwrap().surface.antiship.flights[&m].phase,
            Phase::Intercepted
        );
        assert!(w.contact(Side::Red, cid).is_none());
        // SOURCE EXPECTATION (root fix under test): the armed carrier destruction
        // rolled the printed 50% D100 through the real gun branch.
        let trace: Value = w
            .events
            .iter()
            .find_map(|e| match &e.kind {
                EventKind::RulesEvent { rule, detail, .. } if rule == "14.1.8.carrier_destroyed" => {
                    Some(serde_json::from_str(detail).unwrap())
                }
                _ => None,
            })
            .expect("armed carrier destruction must emit 14.1.8.carrier_destroyed");
        assert_eq!(trace["d100"].as_u64(), Some(u64::from(carrier)));
        assert_eq!(trace["detonates"], json!(true));
        assert_eq!(carrier, carrier.min(50), "seed pinned the detonating branch");
        // Resolve the queued burst exactly as production would on the next tick.
        n::step(&mut w);
        let causes = detonation_causes(&w);
        assert_eq!(causes.len(), 1, "exactly one armed destruction detonation");
        assert_eq!(causes[0], "CarrierDestroyed");
        let state = serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
        assert!(state["bursts"].get(&format!("14.1.8:{}", m.0)).is_some());
        if function == 10 {
            assert!(
                w.entity(ford).unwrap().alive,
                "a D10 function failure (printed '0') must leave every victim untouched"
            );
        } else {
            assert!(
                !w.entity(ford).unwrap().alive,
                "10kt surface burst within the printed 0.41nm ship kill radius must destroy \
                 the defender for game purposes (PDF132/14-6), even though its CIWS won"
            );
        }
        assert!(
            w.entity(launcher).unwrap().alive,
            "the launcher sits 60nm from ground zero, outside the 0.99nm damage radius"
        );
        let saved = serde_json::to_vec(&w).unwrap();
        let restored: World = serde_json::from_slice(&saved).unwrap();
        assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
        return;
    }
    panic!(
        "no seed in {MAX_SEEDS} attempts produced a real source CIWS kill of the armed \
         nuclear carrier; report as fixture precondition, not as the source mechanism"
    );
}

#[test]
fn unarmed_nuclear_ic_destroyed_by_real_source_ford_ciws_stays_silent() {
    for seed in 4_910_001..4_910_001 + MAX_SEEDS {
        let Some((mut w, launcher, ford, m, cid, identity, _carrier, _function, _gun)) =
            engage_once(seed, 80.0, false)
        else {
            continue;
        };
        // SOURCE EXPECTATION (green boundary): enable_run 80nm is never reached across
        // the 60nm flight, the seeker never activates, so the warhead is NOT fuzed when
        // the real gun branch destroys the carrier. No roll, no burst, no detonation.
        assert_eq!(
            fuzed_trace_count(&w),
            0,
            "an unarmed (not yet fuzed) carrier must never emit the arming trace"
        );
        assert!(
            !w.events.iter().any(|e| matches!(
                &e.kind,
                EventKind::RulesEvent { rule, .. } if rule == "14.1.8.carrier_destroyed"
            )),
            "only a fuzed warhead rolls on carrier destruction"
        );
        assert!(detonation_causes(&w).is_empty());
        let state = serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
        assert!(
            state["bursts"].as_object().map_or(true, |o| o.is_empty()),
            "no burst may be queued for an unarmed carrier kill"
        );
        let outcome = &w.rules.aaw.as_ref().unwrap().outcomes[&identity];
        assert!(outcome.hit, "the seed was selected for a real source gun hit");
        assert!(
            !w.entity(m).unwrap().alive,
            "the real intercept must still retire the carrier body"
        );
        assert_eq!(
            w.rules.aaw.as_ref().unwrap().surface.antiship.flights[&m].phase,
            Phase::Intercepted
        );
        assert!(w.contact(Side::Red, cid).is_none());
        assert!(
            w.entity(ford).unwrap().alive,
            "no nuclear effects exist for an unarmed carrier kill"
        );
        assert!(w.entity(launcher).unwrap().alive);
        let saved = serde_json::to_vec(&w).unwrap();
        let restored: World = serde_json::from_slice(&saved).unwrap();
        assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
        return;
    }
    panic!(
        "no seed in {MAX_SEEDS} attempts produced a real source CIWS kill of the unarmed \
         carrier; report as fixture precondition"
    );
}

/// PDF132/14-6 submarine clause through the public game-table API only:
/// full distance underwater, halved from a surface burst against a submerged
/// target; outside the kill radius the submarine damage ring stays pending
/// because the printed D6 sentence covers surface ships only.
#[test]
fn source_1418_submarine_distance_is_full_underwater_and_half_from_surface() {
    assert_eq!(
        n::effect(10, Medium::Subsurface, PlatformKind::Sub, false, false, 0.41).unwrap(),
        Effect::Destroyed
    );
    assert_eq!(
        n::effect(10, Medium::Subsurface, PlatformKind::Sub, false, false, 0.42).unwrap(),
        Effect::Pending(
            "14.1.8 submarine damage-radius casualty not specified by the surface-ship D6 clause"
        )
    );
    assert_eq!(
        n::effect(10, Medium::Surface, PlatformKind::Sub, false, false, 0.205).unwrap(),
        Effect::Destroyed
    );
    assert_eq!(
        n::effect(10, Medium::Surface, PlatformKind::Sub, false, false, 0.21).unwrap(),
        Effect::Pending(
            "14.1.8 submarine damage-radius casualty not specified by the surface-ship D6 clause"
        )
    );
}

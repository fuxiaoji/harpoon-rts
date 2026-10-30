//! Source candidates only. All proposed additions enter through JSON and
//! existing public APIs, so missing support is a normal source-red, not a
//! reference to an unavailable Rust enum. Root owns all Cargo execution.
use harpoon_core::rules::{aircombat2026 as ac, aviation2026 as av};
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};

const FIT: &str = "us_f35c_gbu31_mk84x2";
const BOMB: &str = "us_gbu31_mk84_satnav1";

fn scenario(
    template: &str,
    altitude: f64,
    range: f64,
    course: f64,
    moving: bool,
    allowed: bool,
) -> Value {
    let mut value = json!({
        "id":"pgm-source-2192", "name":"source internal F35C JDAM fit and author allocated fixed briefing",
        "seed":20261026,"duration_s":1000,"weather":{"visibility_nm":40,"jam":[0,0]},
        "context":{
            "rules_profile":"hv2026-rts-v4",
            "flight_environment":{"wind_from_deg":0,"wind_kts":0},
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "aviation_2026":{
                "source":"US Aircraft2025 PDF51/B46 internal2 AIM120+2 GBU31; actual Mk84 SATNAV1 and AIM120D masses give2192kg",
                "loads":{},"parking":{},"initial_fuel_nm":{"STRIKE":1000},
                "surface_aimpoints":{"hangar":{
                    "source":"scenario_assumption: published fixed exercise coordinates and explicit Blue briefing permission; no claim of native airborne Visual at High altitude",
                    "point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]
                }}
            },
            "pgm_2026":{
                "source":"English2026 9.6.8; US Aircraft2025 PDF51/B46,102/H2-4,106/H4-2; exact registered fit IDs only",
                "enabled_fits":[FIT],
                "briefings":{"hangar":{
                    "source":"scenario_assumption: prebriefed exercise target identity, fixed coordinates and motion knowledge allocated to the permitted side",
                    "target":"TARGET","aimpoint":"hangar","known_kind":if moving {"Ship"} else {"Base"},
                    "known_motion":if moving {"moving"} else {"stationary"},
                    "allowed_sides":if allowed {json!(["Blue"])} else {json!(["Red"])},
                    "fcs":{"source":"scenario_assumption: explicitly controlled Good RF quality for the standalone SATNAV physical-chain experiment; not native observation and not a placement result","quality":"Good"}
                }}
            },
            "ground_2026":{"source":"English2026 chapter11 source facility templates; exercise position is an author assumption",
                "facilities":{"TARGET":{"template":template,"custom":null,"source":"English2026 PDF118/11-3 ground source template; no invented size or armor"}}}
        },
        "units":[
            {"side":"Blue","class":FIT,"name":"STRIKE","pos":[0,-range],"course_deg":course,"speed_kts":580,"altitude_m":altitude,"fire_on":false},
            {"side":"Red","class":"airfield","name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}
        ]
    });
    value["context"]["aviation_2026"]["loads"][FIT] = json!({
        "payload_kg":2192,"tanks":[],"external_weapon_rounds":{},
        "internal_weapon_rounds":{"us_gbu31_mk84_satnav1":2,"us_aim120d":2},
        "source":"US2025 B46/H2-4/H4-2: 2*934+2*162kg, internal stores; no optional external AIM9X or gun pod"
    });
    if moving {
        value["units"][1]["class"] = json!("us_burke_2a");
        value["units"][1]["speed_kts"] = json!(20);
        value["context"]
            .as_object_mut()
            .unwrap()
            .remove("ground_2026");
    }
    value
}

fn world(value: Value) -> World {
    let sc: Scenario = serde_json::from_value(value)
        .expect("valid current Scenario JSON; proposed config is tested for retention separately");
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    // Future production source registry must create the exact fitted variant
    // before normal scenario validation. This fixture does not clone World,
    // clear mounts, set private stock, inject contacts or fabricate a flight.
    harpoon_core::load_scenario(sc, db).expect(
        "source registry must supply the exact F35C internal fit and weapon before actual load",
    )
}
fn named(w: &World, name: &str) -> EntityId {
    w.entities
        .values()
        .find(|entity| entity.name == name)
        .unwrap()
        .id
}
fn command(unit: EntityId) -> Command {
    serde_json::from_value(json!({"cmd":"air_combat","unit":unit,
        "action":{"type":"guided_surface_attack","weapon":BOMB,"rounds":1,"briefing":"hangar"}
    }))
    .expect("source 9.6.8 JSON action must be registered; unknown action is a normal source-red")
}
fn inventory(w: &World, actor: EntityId, weapon: &str) -> u32 {
    w.entity(actor)
        .unwrap()
        .weapons
        .iter()
        .filter(|mount| mount.weapon == weapon)
        .map(|mount| mount.mounts as u32 * mount.ammo as u32)
        .sum()
}
fn terminals(w: &World) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "9.6.8.source_terminal" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn p01_exact_internal_fit_is_retained_and_owns_two_source_weapons_without_external_mass_alias() {
    let input = scenario("hangar_medium", 10000., 6., 0., false, true);
    let parsed: Scenario = serde_json::from_value(input.clone()).unwrap();
    let encoded = serde_json::to_value(&parsed).unwrap();
    assert_eq!(
        encoded["context"]["pgm_2026"], input["context"]["pgm_2026"],
        "missing PGM configuration must not silently disappear during Scenario parsing"
    );
    let w = world(input);
    let actor = named(&w, "STRIKE");
    assert_eq!(w.entity(actor).unwrap().class, FIT);
    assert_eq!(inventory(&w, actor, BOMB), 2);
    assert_eq!(inventory(&w, actor, "us_aim120d"), 2);
    assert_eq!(
        w.entity(actor).unwrap().weapons.len(),
        2,
        "this source internal fit has no optional external weapons"
    );
    let flight = av::own_view(&w, actor).unwrap()["flight"].clone();
    assert_eq!(flight["internal_weapon_rounds"][BOMB], 2);
    assert_eq!(flight["internal_weapon_rounds"]["us_aim120d"], 2);
    assert_eq!(flight["external_weapon_rounds"], json!({}));
    assert!((flight["payload_fraction"].as_f64().unwrap() - 2192. / 7715.).abs() < 1e-12);
    let own = ac::own_view(&w, actor).unwrap();
    assert_eq!(own["guided_air_ordnance"]["fit"], FIT);
    assert!(own["guided_air_ordnance"]["briefings"]
        .as_array()
        .unwrap()
        .contains(&json!("hangar")));
    let def = &w.db.weapons[BOMB];
    assert_eq!(
        (def.speed_kts, def.min_range_nm, def.range_nm),
        (528., 2., 13.)
    );
    assert_eq!(def.rule_data.as_ref().unwrap().ship_dp, Some(59.));
    assert_eq!(def.rule_data.as_ref().unwrap().penetration_cm, Some(6.));
}

#[test]
fn p02_actual_briefed_pgm_uses_source_point70_point60_physical_flight_internal_mass_and_restore() {
    // Existing Rust API, proposed action only as JSON: no future enum name.
    let _ = command(EntityId(1));
    for (template, expected) in [("hangar_medium", 0.70), ("hangar_small", 0.60)] {
        let mut w = world(scenario(template, 10000., 6., 0., false, true));
        let actor = named(&w, "STRIKE");
        let target = named(&w, "TARGET");
        assert_eq!(inventory(&w, actor, BOMB), 2);
        let cmd = command(actor);
        harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
        let placement = w
            .events
            .iter()
            .find_map(|e| match &e.kind {
                EventKind::RulesEvent { rule, detail, .. } if rule == "9.6.8.rf_placement" => {
                    Some(serde_json::from_str::<Value>(detail).unwrap())
                }
                _ => None,
            })
            .expect("fixed Base must use the actual cached RF placement executor");
        assert_eq!(
            placement["evidence"]["provenance"]["kind"],
            "scenario_rf_control"
        );
        assert_eq!(placement["evidence"]["provenance"]["native"], false);
        assert!(placement["evidence"]["exemption"].is_null());
        let identity = placement["evidence"]["placement"]["identity"]
            .as_str()
            .unwrap();
        let cached = &w.rules.aaw.as_ref().unwrap().firecontrol.placements[identity];
        assert_eq!(
            serde_json::to_value(cached).unwrap(),
            placement["evidence"]["placement"]
        );
        assert!(cached.placed, "this controlled positive source fixture must actually pass its once-only placement roll");
        assert_eq!(
            inventory(&w, actor, BOMB),
            1,
            "one real internal bomb is physically released once"
        );
        let flight = av::own_view(&w, actor).unwrap()["flight"].clone();
        assert_eq!(flight["internal_weapon_rounds"][BOMB], 1);
        assert_eq!(flight["external_weapon_rounds"], json!({}));
        assert!(
            (flight["payload_fraction"].as_f64().unwrap() - 1258. / 7715.).abs() < 1e-12,
            "actual internal release removes934kg, preserving the other bomb and two AIM120D"
        );
        let projectile = w
            .entities
            .values()
            .find(|entity| {
                entity.alive && entity.projectile.as_ref().is_some_and(|p| p.weapon == BOMB)
            })
            .expect(
                "source528kt guided bomb must exist physically before its six-mile flight can end",
            )
            .clone();
        assert_eq!(projectile.speed_kts, 528.);
        assert!(
            terminals(&w).is_empty(),
            "source weapon cannot resolve instantly at standoff range"
        );
        let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
        let moved = w
            .entity(projectile.id)
            .unwrap()
            .pos
            .range_to(projectile.pos);
        assert!((moved-528./3600.).abs()<1e-9,"one native second consumes one source speed step; no speed multiplier or double advance");
        for _ in 0..100 {
            if !terminals(&w).is_empty() {
                break;
            }
            harpoon_core::sim::tick(&mut w, &[]);
            harpoon_core::sim::tick(&mut restored, &[]);
        }
        let result = terminals(&w);
        assert_eq!(
            result.len(),
            1,
            "single source terminal result, no generic Bomb/pk fallback"
        );
        assert_eq!(
            result[0]["probability"], expected,
            "PGM table depends on physical target size; unguided NoAA shifts do not apply"
        );
        assert_eq!(result[0]["damage_points_per_hit"], 59.);
        assert_eq!(result[0]["penetration_cm"], 6.);
        assert_eq!(result[0]["weapon"], BOMB);
        let hit = result[0]["hit"].as_bool().unwrap();
        let damage = w.rules.ground.as_ref().unwrap().facilities[&target].damage;
        assert!(
            (damage - if hit { 59. } else { 0. }).abs() < 1e-9,
            "unarmored source facility receives actual Mk84 DP only on a source hit"
        );
        assert_eq!(inventory(&w, actor, BOMB), 1);
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            serde_json::to_value(&restored).unwrap()
        );
    }
}

#[test]
fn p03_known_moving_target_or_illegal_satnav1_envelope_never_spends_internal_stock() {
    let _ = command(EntityId(1));
    for (altitude, range, course, moving, allowed) in [
        (6000., 6., 0., false, true),
        (10000., 14., 0., false, true),
        (10000., 1., 0., false, true),
        (10000., 6., 31., false, true),
        (10000., 6., 0., true, true),
        (10000., 6., 0., false, false),
    ] {
        let mut w = world(scenario(
            "hangar_medium",
            altitude,
            range,
            course,
            moving,
            allowed,
        ));
        let actor = named(&w, "STRIKE");
        let cmd = command(actor);
        harpoon_core::sim::tick(&mut w, &[(Side::Blue, cmd)]);
        assert_eq!(
            inventory(&w, actor, BOMB),
            2,
            "illegal source request cannot spend physical internal bombs"
        );
        let flight = av::own_view(&w, actor).unwrap()["flight"].clone();
        assert_eq!(flight["internal_weapon_rounds"][BOMB], 2);
        assert!(w
            .entities
            .values()
            .all(|entity| !entity.alive
                || !entity.projectile.as_ref().is_some_and(|p| p.weapon == BOMB)));
        assert!(terminals(&w).is_empty());
        assert!(
            w.events.iter().any(
                |event| matches!(&event.kind,EventKind::RulesEvent{domain,rule,..}
            if domain=="command"&&rule=="rejected")
            ),
            "source rejection must be an actual normal command receipt"
        );
        if !allowed {
            let own = ac::own_view(&w, actor).unwrap();
            assert!(
                !own["guided_air_ordnance"]["briefings"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("hangar")),
                "briefing allocation is own-side permission, not target truth"
            );
        }
    }
}

#[test]
fn p04_bare_launch_and_ordinary_fire_cannot_bypass_the_source_briefing_permit() {
    let mut input = scenario("hangar_medium", 10000., 6., 0., false, true);
    input["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":"us_ford","name":"LOOKOUT","pos":[0,-0.25],"speed_kts":0,"fire_on":false}));
    input["context"]["detection_2026"] = json!({
        "bindings":{"us_ford:visual":{"type":"visual","source":"English2026 5.8 real fitted human lookout"}},
        "targets":{"airfield":{"source":"English2026 PDF118 Medium Hangar physicalD assigned to this actual fixture component","size":3,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":null}},
        "environment":{"source":"scenario_assumption: clear daylight native visual permit-bypass fixture","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":2000,"layer":null,"ice":0,"cloud":null}
    });
    let mut w = world(input);
    harpoon_core::sim::tick(&mut w, &[]);
    let actor = named(&w, "STRIKE");
    let target = named(&w, "TARGET");
    let contact = w
        .find_contact(Side::Blue, target)
        .expect("normal Fire fixture must have a real native visual target contact");
    assert!(w
        .contact(Side::Blue, contact)
        .unwrap()
        .sensors
        .iter()
        .any(|sensor| sensor == "visual"));
    let def = w.db.weapons[BOMB].clone();
    let before = serde_json::to_value(&w).unwrap();
    assert!(
        harpoon_core::weapons::launch(
            &mut w,
            actor,
            &def,
            Some(target),
            Some(harpoon_core::Vec2::zero())
        )
        .is_none(),
        "a bare source-PGM launch has no private release permit"
    );
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        before,
        "failed bare source launch is read-only"
    );
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: actor,
                contact,
                weapon: BOMB.into(),
                rounds: 1,
            },
        )],
    );
    assert_eq!(inventory(&w, actor, BOMB), 2);
    let flight = av::own_view(&w, actor).unwrap()["flight"].clone();
    assert_eq!(flight["internal_weapon_rounds"][BOMB], 2);
    assert!((flight["payload_fraction"].as_f64().unwrap() - 2192. / 7715.).abs() < 1e-12);
    assert!(w.entities.values().all(
        |entity| !entity.alive || !entity.projectile.as_ref().is_some_and(|p| p.weapon == BOMB)
    ));
}

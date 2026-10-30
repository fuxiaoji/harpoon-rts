//! Source-red-first cases for RF/placement and the common aircraft budget.
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};
use std::collections::BTreeSet;
const FIT: &str = "us_f35c_gbu31_mk84x2";
const BOMB: &str = "us_gbu31_mk84_satnav1";

fn input(pgm_count: usize, mk82: bool, ship: bool, measured: bool) -> Value {
    let settling = if measured { 181. } else { 0. };
    let mut value = json!({"id":"pgm-policy-source","name":"PGM source placement and common aircraft budget","seed":6619,"duration_s":1000,
        "weather":{"visibility_nm":40,"jam":[0,0]},"context":{
            "rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},"radar_environment":{"sea_state":0,"precipitation":"None"},
            "aviation_2026":{"source":"US2025 B46 exact internal fit; scenario source-policy exercise coordinates",
                "loads":{"us_f35c_gbu31_mk84x2":{"payload_kg":2192,"tanks":[],"external_weapon_rounds":{},"internal_weapon_rounds":{"us_gbu31_mk84_satnav1":2,"us_aim120d":2},"source":"US2025 B46/H2-4/H4-2 exact internal2192kg"}},
                "parking":{},"initial_fuel_nm":{},"no_aa_surface_attack_source":"scenario_assumption: controlled weapons-hold exercise; actual AA would override",
                "surface_aimpoints":{"range":{"source":"scenario_assumption: allocated fixed test coordinates; no authored FCS quality or placement result","point":{"x":0,"y":0},"controlled_range":true,"fixed_target":true,"allowed_sides":["Blue"]}}
            },
            "pgm_2026":{"source":"English2026 9.6.3/9.6.8 explicit source-policy candidate; no claim that fixed author coordinates exempt placement","enabled_fits":[FIT],
                "briefings":{"range":{"source":"scenario_assumption: target identity/position allocation only, never a Good/Fair/Poor solution","target":"TARGET","aimpoint":"range","known_kind":if ship{"Ship"}else{"Base"},"known_motion":"stationary","allowed_sides":["Blue"]}},
                "countermeasures":if ship{json!({"TARGET":{"source":"scenario_assumption: controlled0CM ship-budget fixture, not a real GPS-jammer fit claim","generation":0}})}else{json!({})}
            }
        },"units":[]});
    for i in 0..pgm_count {
        let name = format!("PGM{i}");
        value["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":FIT,"name":name,"pos":[0.,-6.-580.*settling/3600.],"course_deg":0.,"speed_kts":580.,"altitude_m":10000.,"fire_on":false}));
        value["context"]["aviation_2026"]["initial_fuel_nm"][&name] = json!(1000);
    }
    if mk82 {
        value["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":"us_f18e_mk82","name":"IRON","pos":[0.,-460.*settling/3600.],"course_deg":0.,"speed_kts":460.,"altitude_m":1000.,"fire_on":false}));
        value["context"]["aviation_2026"]["loads"]["us_f18e_mk82"] = json!({"payload_kg":6395,"tanks":[{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"}],"external_weapon_rounds":{"us_mk82":6},"source":"US2025 B44/H1-3/H4-2 exact six-Mk82 load"});
        value["context"]["aviation_2026"]["initial_fuel_nm"]["IRON"] = json!(1000);
    }
    value["units"].as_array_mut().unwrap().push(json!({"side":"Red","class":if ship{"us_burke_2a"}else{"airfield"},"name":"TARGET","pos":[0.,0.],"speed_kts":0.,"fire_on":false}));
    if !ship {
        value["context"]["ground_2026"] = json!({"source":"English2026 PDF118 source Medium Hangar physicalD/100DP/unarmored","facilities":{"TARGET":{"template":"hangar_medium","custom":null,"source":"English2026 PDF118/11-3"}}});
    }
    if measured {
        value["context"]["detection_2026"] = json!({
            "targets":{"us_burke_2a":{"source":"scenario_assumption: controlled radar return for native FCS/budget fixture; physicalB is the source hull size","size":1,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}},
            "environment":{"source":"scenario_assumption: clear daylight continuous native radar measurements","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}
        });
    }
    value
}
fn world(value: Value) -> World {
    harpoon_core::load_scenario(
        serde_json::from_value::<Scenario>(value).unwrap(),
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap(),
    )
    .unwrap()
}
fn named(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn pgm(w: &World, id: EntityId) -> (Side, Command) {
    (Side::Blue,serde_json::from_value(json!({"cmd":"air_combat","unit":id,"action":{"type":"guided_surface_attack","weapon":BOMB,"rounds":1,"briefing":"range"}})).unwrap())
}
fn is_terminal(e: &harpoon_core::events::Event) -> bool {
    matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="9.6.8.source_terminal")
}

#[test]
fn policy01_fixed_base_brief_without_native_fcs_cannot_silently_skip_source_placement() {
    let mut w = world(input(1, false, false, false));
    let actor = named(&w, "PGM0");
    let target = named(&w, "TARGET");
    harpoon_core::sim::tick(&mut w, &[]);
    assert!(
        harpoon_core::rules::firecontrol2026::own_world_solution(&w, actor, target, 1, 528.)
            .is_err(),
        "fixture must truly lack an observer-qualified solution; author coordinates are not one"
    );
    let cmd = pgm(&w, actor);
    harpoon_core::sim::tick(&mut w, &[cmd]);
    for _ in 0..100 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(!w.events.iter().any(is_terminal),"English108/109/116 supplies no explicit fixed-Base placement exemption; unresolved scope must remain specific pending, not a complete PGM Ph roll");
    assert_eq!(
        w.rules.ground.as_ref().unwrap().facilities[&target].damage,
        0.
    );
    assert!(
        w.events.iter().any(|e| match &e.kind {
            EventKind::RulesEvent {
                domain,
                rule,
                detail,
                ..
            } if domain == "pgm2026" && rule.contains("pending") => {
                let lower = detail.to_ascii_lowercase();
                lower.contains("placement") || lower.contains("fcs") || lower.contains("rf")
            }
            _ => false,
        }),
        "pending must identify the unresolved RF/FCS/placement source condition"
    );
}

#[test]
fn policy02_pgm_and_mk82_share_the_same_ship_tactical_turn_twelve_aircraft_budget() {
    for (count, mixed) in [(13, false), (12, false), (12, true), (11, true)] {
        let mut w = world(input(count, mixed, true, true));
        for _ in 0..181 {
            harpoon_core::sim::tick(&mut w, &[]);
        }
        let target = named(&w, "TARGET");
        let actors: Vec<_> = w
            .entities
            .values()
            .filter(|e| {
                e.side == Side::Blue && e.kind == harpoon_core::types::PlatformKind::Aircraft
            })
            .map(|e| e.id)
            .collect();
        for actor in actors
            .iter()
            .filter(|id| w.entity(**id).unwrap().class == FIT)
        {
            assert!(w.rules.aaw.as_ref().unwrap().firecontrol.tracks.values().any(|t|
                t.observer==*actor&&t.target==target&&t.report.as_ref().is_some_and(|r|r.at==w.tick&&w.db.sensors.get(&r.sensor).is_some_and(|s|s.kind==harpoon_core::db::SensorKind::Radar))&&t.motion.as_ref().is_some_and(|m|m.known&&m.last==w.tick)
            ),"budget fixture needs real fresh native radar and a completed motion window; no private FCS injection");
        }
        let mut commands: Vec<_> = actors
            .iter()
            .filter(|id| w.entity(**id).unwrap().class == FIT)
            .map(|id| pgm(&w, *id))
            .collect();
        if mixed {
            let actor = named(&w, "IRON");
            let contact = w
                .find_contact(Side::Blue, target)
                .expect("actual native own target contact");
            commands.push((Side::Blue,serde_json::from_value(json!({"cmd":"air_combat","unit":actor,"action":{"type":"surface_attack","contact":contact,"weapon":"us_mk82","rounds":6,"delivery":"Glide","aimpoint":"range"}})).unwrap()));
        }
        let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        let mut reversed: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        let mut reverse_commands = commands.clone();
        reverse_commands.reverse();
        harpoon_core::sim::tick(&mut w, &commands);
        harpoon_core::sim::tick(&mut restored, &commands);
        harpoon_core::sim::tick(&mut reversed, &reverse_commands);
        let released: BTreeSet<_> = w
            .events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::WeaponsRelease { shooter, .. } if actors.contains(shooter) => {
                    Some(*shooter)
                }
                _ => None,
            })
            .collect();
        let reverse_released: BTreeSet<_> = reversed
            .events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::WeaponsRelease { shooter, .. } if actors.contains(shooter) => {
                    Some(*shooter)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            released, reverse_released,
            "client command order must not choose which domain/aircraft owns the common budget"
        );
        for actor in &actors {
            let stocks = |world: &World| {
                world
                    .entity(*actor)
                    .unwrap()
                    .weapons
                    .iter()
                    .map(|m| (m.weapon.clone(), m.mounts, m.ammo))
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                stocks(&w),
                stocks(&reversed),
                "reversed mixed declarations must preserve the same actual stock debit"
            );
        }
        let requested = count + usize::from(mixed);
        if requested == 12 {
            assert_eq!(released.len(),12,"source-valid twelve-aircraft control must actually release across the common phase");
        }
        assert!(released.len()<=12,"PGM and iron bombs cannot each acquire their own twelve-aircraft ship budget; requested{requested}, released{}",released.len());
        let turn = w.tick.div_euclid(180);
        let key = format!("{}:{turn}", target.0);
        let budget = w
            .rules
            .aircombat
            .as_ref()
            .unwrap()
            .surface_attacks
            .get(&key);
        if !released.is_empty() {
            assert!(
                budget.is_some_and(|set| released.iter().all(|id| set.contains(id))),
                "the real common ledger must contain every released PGM/Mk82 aircraft"
            );
        }
        assert!(budget.is_none_or(|set| set.len() <= 12));
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            serde_json::to_value(&restored).unwrap()
        );
    }
}

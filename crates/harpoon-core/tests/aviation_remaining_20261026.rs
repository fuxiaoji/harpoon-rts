//! Source candidates: English 2026 §4.8 PDF38–39 and US Aircraft
//! 2025-12-01 PDF51/B46, PDF64/B59, PDF91/B82.  Airfield geometry,
//! declared loads and remaining fuel are explicit scenario assumptions.
use harpoon_core::rules::aviation2026::{self as aviation, Action, RefuelMethod};
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::json;

fn world() -> World {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"aviation-remaining-source", "name":"KC130J source refueling",
        "duration_s":2000, "seed":419,
        "context":{
            "rules_profile":"hv2026-rts-v4",
            "flight_environment":{"wind_from_deg":0,"wind_kts":0},
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "air_wings":{"Base":[{"class":"us_kc130j","count":1},{"class":"us_f35b","count":2}]},
            "aviation_2026":{
                "source":"scenario_assumption: refueling exercise, declared light load and initial remaining fuel",
                "loads":{
                    "us_kc130j":{"payload_kg":0,"tanks":[],"source":"scenario_assumption: standard external tanks already included in source base range; no cabin tank"},
                    "us_f35b":{"payload_kg":3000,"tanks":[],"source":"scenario_assumption: light exercise payload retaining physical DB weapon mounts; no offensive action or exact combat fit is being validated"}
                },
                "initial_fuel_nm":{"Tanker":400,"Receiver1":200,"Receiver2":200},
                "parking":{"Base":0}
            },
            "ground_2026":{
                "source":"scenario_assumption: explicit exercise recovery airfield",
                "facilities":{"Runway":{"template":"runway","custom":null,"source":"scenario_assumption","parent":"Base","runway":{"length_m":3000,"baseline":"Large"}}},
                "air_services":{"Base":{
                    "source":"scenario_assumption: return reserve requires explicit recovery capability",
                    "launch_lanes":1,"landing_lanes":1,"launch_interval_s":60,"landing_interval_s":60,
                    "heading_deg":0,"initial_speed_kts":{"us_kc130j":200,"us_f35b":200},
                    "landing_speed_kts":{"us_kc130j":200,"us_f35b":200},
                    "turnaround_s":{"us_kc130j":600,"us_f35b":600},
                    "landing_min_altitude_m":61,"landing_max_altitude_m":2000,
                    "landing_speed_tolerance_kts":1,"landing_heading_tolerance_deg":1
                }}
            }
        },
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "units":[
            {"side":"Blue","class":"airfield","name":"Base","pos":[0,0]},
            {"side":"Blue","class":"airfield","name":"Runway","pos":[0,0]},
            {"side":"Blue","class":"us_kc130j","name":"Tanker","pos":[0,0],"speed_kts":320,"altitude_m":6000},
            {"side":"Blue","class":"us_f35b","name":"Receiver1","pos":[0,0],"speed_kts":320,"altitude_m":6000},
            {"side":"Blue","class":"us_f35b","name":"Receiver2","pos":[0,0],"speed_kts":320,"altitude_m":6000}
        ]
    })).unwrap();
    harpoon_core::load_scenario(scenario, db).unwrap()
}

fn named(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}

#[test]
fn printed_kc130j_f35b_fits_enable_the_actual_two_hose_example() {
    let tanker = aviation::spec("us_kc130j").expect("PDF64/B59 KC130J needs a real source flight profile");
    let receiver = aviation::spec("us_f35b").expect("PDF51/B46 F35B needs a real source flight profile");
    assert_eq!((tanker.tanker_slots, tanker.size, tanker.centerline), (2, 3, false));
    assert_eq!((tanker.cruise_kts, tanker.cruise_range_nm, tanker.fuel_kg), (320., 3485., 29380.));
    assert_eq!((receiver.size, receiver.cruise_range_nm, receiver.fuel_kg), (1, 870., 6125.));
    assert_eq!(receiver.receive, Some(RefuelMethod::WesternProbe));
    assert_eq!(aviation::offload_factor("us_kc130j", "us_f35b").unwrap(), 1.4);
    assert_eq!(aviation::refuel_seconds(receiver.size, RefuelMethod::WesternProbe).unwrap(), 540);
    let mut w = world();
    let ids = [named(&w, "Tanker"), named(&w, "Receiver1"), named(&w, "Receiver2")];
    assert!(ids.iter().all(|id| w.rules.aviation.as_ref().unwrap().flights.contains_key(id)));
    assert!(harpoon_core::air::compatible_recovery(&w, ids[0], named(&w, "Base")));
    assert!(aviation::apply(&mut w, ids[0], &Action::Refuel {
        receiver:ids[1], method:RefuelMethod::BuddyWestern
    }).is_err(), "N/2P is a dedicated Western probe tanker; its hoses cannot be relabeled as a buddy store");
}

fn exercise(reverse: bool) -> World {
    let mut w = world();
    let tanker = named(&w, "Tanker");
    let receivers = if reverse { ["Receiver2", "Receiver1"] } else { ["Receiver1", "Receiver2"] };
    let commands: Vec<_> = receivers.iter().map(|name| (Side::Blue, Command::Aviation {
        unit:tanker, action:Action::Refuel { receiver:named(&w, name), method:RefuelMethod::WesternProbe }
    })).collect();
    harpoon_core::sim::tick(&mut w, &commands);
    let jobs = &w.rules.aviation.as_ref().unwrap().refuels;
    assert_eq!(jobs.len(), 2, "PDF39 example4 uses two hoses concurrently");
    assert_eq!(jobs[0].due, jobs[1].due);
    let due = jobs[0].due;
    assert_eq!(due - 1, 540);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    while w.tick < due {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(serde_json::to_value(&w).unwrap(), serde_json::to_value(&restored).unwrap());
    w
}

#[test]
fn actual_two_hoses_share_finite_return_safe_fuel_without_command_order_advantage() {
    let normal = exercise(false);
    let reverse = exercise(true);
    let s = normal.rules.aviation.as_ref().unwrap();
    let reversed = reverse.rules.aviation.as_ref().unwrap();
    let tanker = named(&normal, "Tanker");
    let mut delivered = Vec::new();
    for name in ["Receiver1", "Receiver2"] {
        let id = named(&normal, name);
        let fuel = s.flights[&id].fuel_nm;
        assert!(fuel > 200., "both simultaneously connected receivers must get their share");
        assert!((fuel - reversed.flights[&id].fuel_nm).abs() < 1e-8, "network command order cannot choose which connected receiver gets scarce fuel");
        delivered.push(fuel);
    }
    assert!((delivered[0] - delivered[1]).abs() < 1e-8, "equal source needs share the same pump budget equally");
    let distance = normal.entity(tanker).unwrap().pos.range_to(normal.entity(named(&normal, "Base")).unwrap().pos);
    assert!(s.flights[&tanker].fuel_nm + 1e-8 >= distance, "the shared debit must preserve the current tanker return range");
    // One unit of tanker cruise range transfers 1.4 units of receiver range.
    // All three fly the same real path and lose that range before completion.
    let conserved = s.flights[&tanker].fuel_nm + delivered.iter().sum::<f64>() / 1.4;
    let initial = 400. + 400. / 1.4;
    assert!((conserved - (initial - distance * (1. + 2. / 1.4))).abs() < 1e-7);
    assert_eq!(normal.events.iter().filter(|event| matches!(&event.kind,
        harpoon_core::EventKind::RulesEvent{rule,..} if rule == "4.8.refuel_completed")).count(), 2);
}

#[test]
fn hifr_is_interrupted_when_the_normal_command_stream_assigns_a_combat_target() {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"hifr-combat-interruption", "name":"HIFR combat interruption", "duration_s":1000,"seed":19,
        "context":{
            "rules_profile":"hv2026-rts-v4",
            "flight_environment":{"wind_from_deg":0,"wind_kts":0},
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "air_wings":{"Ship":[{"class":"us_mh60r","count":1}]},
            "aviation_2026":{"source":"scenario_assumption: HIFR exercise with physical weapons retained",
                "loads":{"us_mh60r":{"payload_kg":600,"tanks":[],"source":"scenario_assumption: 600kg light exercise payload retaining actual DB Mk54 mounts; no exact combat load is being validated"}},
                "parking":{"Ship":0},"initial_fuel_nm":{"Helicopter":200}},
            "detection_2026":{
                "bindings":{
                    "us_mh60r:visual":{"type":"visual","source":"English2026 5.8 human lookout; actual DB visual sensor retained"},
                    "us_ford:visual":{"type":"visual","source":"English2026 5.8 human lookout; actual DB visual sensor retained"}
                },
                "environment":{"source":"scenario_assumption: clear daylight close-range HIFR exercise",
                    "sea_state":0,"precipitation":0,"visibility_percent":100,
                    "shipping":0,"biologics":0,"water_depth_m":1000,"layer":null,"ice":0,"cloud":null}
            }
        },
        "weather":{"visibility_nm":40,"jam":[0,0]},
        "units":[
            {"side":"Blue","class":"us_ford","name":"Ship","pos":[0,0],"speed_kts":32,"fire_on":false},
            {"side":"Blue","class":"us_mh60r","name":"Helicopter","pos":[0,0],"speed_kts":32,"altitude_m":15,"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"Opponent","pos":[0,0.25],"speed_kts":0,"fire_on":false}
        ]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let helicopter = named(&w, "Helicopter");
    let ship = named(&w, "Ship");
    let opponent = named(&w, "Opponent");
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, Command::Aviation {
        unit:helicopter, action:Action::Hifr { ship }
    })]);
    assert!(w.rules.aviation.as_ref().unwrap().hifr.contains_key(&helicopter), "fixture must reach real HIFR pending state");
    let contact = w.find_contact(Side::Blue, opponent).expect("actual native visual scan must discover this close, known-size surface ship");
    let observed = w.contact(Side::Blue, contact).unwrap();
    assert!(!observed.stale && !observed.bearing_only);
    assert!(observed.sensors.iter().any(|sensor| sensor == "visual"), "combat assignment must use actual fitted Visual evidence");
    assert_eq!(harpoon_core::rules::detection2026::known_category(&w, Side::Blue, opponent), Some(harpoon_core::types::PlatformKind::Ship));
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, Command::AssignTarget {unit:helicopter,contact})]);
    assert_eq!(w.entity(helicopter).unwrap().assigned, Some(contact), "normal player combat task was actually accepted");
    assert!(!w.rules.aviation.as_ref().unwrap().hifr.contains_key(&helicopter),
        "§4.9 requires noncombat throughout the evolution, not just on the initial admission tick");
    assert!(w.rules.aviation.as_ref().unwrap().flights[&helicopter].fuel_nm < 200.);
    assert!(!w.rules.aviation.as_ref().unwrap().hifr_used.contains(&helicopter));
}

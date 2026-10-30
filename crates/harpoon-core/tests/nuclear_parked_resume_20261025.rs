//! Source PDF131/14-5 notes1/2: an explicitly stored ground aircraft is not
//! an in-flight 4psi target. Laboratory nuclear delivery/terminal inputs below
//! are explicit source fixtures, never a claim of a nuclear ESSM real fitting.
//! No contact, die, HP, Flight or runtime casualty flag is injected.
use harpoon_core::{
    rules::{aaw2026 as a, nuclear2026 as n},
    Database, EntityId, Rng, Scenario, Side, World,
};
use serde_json::json;

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn fixture(distance: f64) -> (World, EntityId, EntityId, EntityId) {
    fixture_with_missile(distance,false)
}
fn fixture_with_missile(distance: f64, airborne_missile: bool) -> (World, EntityId, EntityId, EntityId) {
    // Source terminal random streams are selected, not overwritten. Five
    // explicit scenario units precede the real allocated carrier ID6.
    let seed = (0..100000)
        .find(|s| {
            let mut function = Rng::new(*s).derive(Rng::hash_tag(0, 0, "14.1.8:6:function"));
            let mut terminal = Rng::new(*s).derive(Rng::hash_tag(0, 1, "aaw:1:us_essm_i:2:6"));
            1 + (function.next_f64() * 10.0).floor() as u8 != 10
                && 1 + (terminal.next_f64() * 100.0).floor() as u8 <= 35
        })
        .unwrap();
    let sc:Scenario=serde_json::from_value(json!({"id":"nuclear-parked-native","name":"scenario_assumption: source parked/inflight radius experiment","duration_s":1000,"seed":seed,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","nuclear_2026":{"source":"scenario_assumption: laboratory nuclear delivery only","units":{"LAB":{"us_essm_i":{"source":"scenario_assumption: carrier nuclear lifecycle laboratory; no historical fitting","source_variant":"scenario_assumption:parked_radius_lab","allowed_yields_kt":[1],"selected_yield_kt":1,"medium":"surface","fuze":"free_fall","authorized":true}}}},
            "ground_2026":{"source":"scenario_assumption: explicit individual ground parking","facilities":{"PARK":{"template":null,"custom":{"size":"F","damage_points":null,"vertical_armor":0,"horizontal_armor":0,"effect":"aircraft_one","source":"scenario_assumption: parked apron"},"source":"scenario_assumption: parked apron","stored_aircraft":["PARKED"]}}}},
        "units":[{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"us_f18e","name":"TRIGGER","pos":[0,0.01],"speed_kts":200,"altitude_m":1000,"fire_on":false},
            {"side":"Blue","class":"airfield","name":"PARK","pos":[distance,0],"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":"us_f18e","name":"PARKED","pos":[distance,0],"speed_kts":0,"altitude_m":0,"fire_on":false},
            {"side":"Blue","class":"us_f18e","name":"FLYING","pos":[distance,0],"speed_kts":200,"altitude_m":1000,"fire_on":false}]})).unwrap();
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let b = id(&w, "LAB");
    let t = id(&w, "TRIGGER");
    let parked = id(&w, "PARKED");
    let flying = id(&w, "FLYING");
    let def = w.db.weapons["us_essm_i"].clone();
    let carrier = harpoon_core::weapons::launch(&mut w, b, &def, Some(t), None)
        .expect("actual laboratory delivery required");
    assert_eq!(carrier.0, 6);
    let attack = a::MissileAttack {
        ata: 2.0,
        maneuver_rating: 0.0,
        maneuvering: false,
        speed_kts: 2003.0,
        signature: 2,
        combat_generation: 4,
        automatic: false,
        target_is_missile: true,
        range_nm: 30.0,
        bearing: a::Bearing::Closing,
        terminal_maneuvers: false,
        sea_skimming: false,
        sea_capability: 2,
        noe: false,
        manpads_or_salh: false,
        high_diving: false,
        land_clutter: false,
        radar_guided: true,
        countermeasures: None,
        hoj: false,
    };
    let budget =
        a::intercept_budget(4, false, 16000.0, 2003.0, 68.8, 1650.0, 3.5, false, None).unwrap();
    a::commit(
        w.rules.aaw.as_mut().unwrap(),
        a::Permit {
            key: "scenario_assumption: frozen source terminal input".into(),
            channel: 0,
            shooter: b,
            target: t,
            weapon: "us_essm_i".into(),
            tick: 0,
            band: a::Band::Long1,
            budget,
            initial_course: 0.0,
            initial_range: 30.0,
            attack,
            missiles: 1,
        },
        &[carrier],
    )
    .unwrap();
    harpoon_core::weapons::advance_projectiles(&mut w);
    assert!(
        w.rules
            .aaw
            .as_ref()
            .unwrap()
            .outcomes
            .values()
            .next()
            .unwrap()
            .hit,
        "real source terminal checkpoint required"
    );
    if airborne_missile {
        let conventional=w.db.weapons["us_aim120d"].clone();
        harpoon_core::weapons::launch(&mut w,flying,&conventional,None,Some(harpoon_core::Vec2::new(100.,0.))).expect("actual installed conventional missile body");
    }
    n::step(&mut w);
    assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,detail,..} if rule=="14.1.8.detonation"&&detail.contains("\"failed\":false"))),"real non-dud function checkpoint required");
    (w, parked, flying, carrier)
}

#[test]
fn an_inflight_missile_in_the_two_psi_ring_really_aborts_its_flight() {
    let (mut w,_,_,_)=fixture_with_missile(0.35,true);
    let id=w.entities.values().find(|e|e.projectile.as_ref().is_some_and(|p|p.weapon=="us_aim120d")).unwrap().id;
    assert!(!w.entity(id).unwrap().alive,"a mission-abort marker alone must not leave the physical missile flying into an attack");
    let pos=w.entity(id).unwrap().pos;
    harpoon_core::weapons::advance_projectiles(&mut w);
    assert_eq!(w.entity(id).unwrap().pos,pos);
}

#[test]
fn parked_ground_aircraft_between_eight_and_four_psi_survives_but_cannot_continue_mission() {
    let (w, parked, flying, _) = fixture(0.24);
    let grounded = w
        .entity(parked)
        .expect("source ground aircraft cannot inherit in-flight destruction");
    assert!(
        grounded.alive,
        "PDF131 note1 ground/deck aircraft are outside the 1kt 8psi .19nm kill radius"
    );
    assert_eq!(
        grounded.ext.get("nuclear.mission_abort"),
        Some(&1.0),
        "2psi damage-range mission abort is required; no aircraft DP formula is invented"
    );
    assert!(
        w.entity(flying).is_none_or(|e| !e.alive),
        "the genuinely in-flight aircraft is inside its .29nm 4psi kill radius"
    );
}

#[test]
fn explicitly_parked_aircraft_inside_eight_psi_still_receives_source_destruction() {
    let (w, parked, _, _) = fixture(0.10);
    assert!(w.entity(parked).is_none_or(|e| !e.alive));
}

#[test]
fn surface_burst_damage_ring_aborts_both_parked_and_inflight_aircraft_without_ship_dp_roll() {
    let (w, parked, flying, _) = fixture(0.35);
    for aircraft in [parked, flying] {
        let e = w.entity(aircraft).unwrap();
        assert!(e.alive);
        assert_eq!(e.ext.get("nuclear.mission_abort"), Some(&1.0));
        assert_eq!(
            e.hp, e.hp_max,
            "surface-ship D6x10% DP is not a grounded-aircraft damage formula"
        );
    }
    let copy: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_vec(&w).unwrap(),
        serde_json::to_vec(&copy).unwrap()
    );
}

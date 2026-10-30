//! One bounded natural source SAM stage candidate. No Contact/HP/shots/ledger injection.
use harpoon_core::rules::{aaw2026 as aa, aircombat2026 as ac, operations};
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, Vec2, World};
use serde_json::json;
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn stock(w: &World, a: EntityId) -> u16 {
    w.entity(a)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_mk82")
        .unwrap()
        .ammo
}
fn run(w: &mut World, to: i64) {
    while w.tick < to {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn scenario_seed() -> u64 {
    (0..1_000_000)
        .find(|seed| {
            let mut r = Rng::new(*seed).derive(Rng::hash_tag(91, 3, "aaw:3:us_essm_i:1:4"));
            1 + (r.next_f64() * 100.) as u8 == 100
        })
        .unwrap()
}
fn fixture() -> World {
    let target = |size| json!({"source":"scenario_assumption: explicit controlled native visual profile","size":size,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":3});
    let sc:Scenario=serde_json::from_value(json!({"id":"wide-natural-sam-bombing","name":"scenario_assumption: bounded native radar reaction to real SAM; held original bombing aim","seed":scenario_seed(),"duration_s":1000,"weather":{"visibility_nm":40,"jam":[0,0]},"context":{
  "rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
  "aviation_2026":{"source":"US Aircraft2025 B44/H1-3 exact strike fit; controlled initial fuel",
   "loads":{"us_f18e_mk82":{"payload_kg":6395,"tanks":[{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"},{"range_add_nm":290,"fuel_weight_kg":1485,"source":"US2025 B44"}],"external_weapon_rounds":{"us_mk82":6},"source":"US2025 B44/H4-2/H1-3: actual cited Mk82 strike load"}},"parking":{},"initial_fuel_nm":{"STRIKE":1000}},
  "detection_2026":{"bindings":{"us_f18e_mk82:visual":{"type":"visual","source":"scenario_assumption: fitted lookout"}},"targets":{"us_f18e_mk82":target(5),"cn_001":target(0),"us_ford":target(0)},"environment":{"source":"scenario_assumption: clear daylight; original source Ford radar fit/clock unchanged","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
  "units":[{"side":"Blue","class":"us_f18e_mk82","name":"STRIKE","pos":[0,-91.0*460.0/3600.0],"course_deg":0,"speed_kts":460,"altitude_m":1000,"fire_on":false},
   {"side":"Red","class":"cn_001","name":"TARGET","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false},
   {"side":"Red","class":"us_ford","name":"DEFENSE","pos":[0,5],"course_deg":180,"speed_kts":0,"fire_on":false}]})).unwrap();
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    ac::configure_database(&mut db);
    db.platforms.get_mut("cn_001").unwrap().weapons.clear();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let defense = id(&w, "DEFENSE");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Red,
            Command::SetRadar {
                unit: defense,
                active: true,
            },
        )],
    );
    run(&mut w, 91);
    w
}
#[test]
fn sam01_real_source_terminal_waits_for_the_next_normal_gun_scan_and_does_not_reuse_flown_past_geometry(
) {
    let mut w = fixture();
    let (a, d, t) = (id(&w, "STRIKE"), id(&w, "DEFENSE"), id(&w, "TARGET"));
    assert!(w.entity(a).unwrap().pos.range_to(Vec2::zero())<1e-9,"controlled initial source position/speed naturally reaches the native observed aim at tick91");
    assert!(
        operations::native_air_target_ready(&w, d, a, "radar_air_long"),
        "original native Ford radar must actually earn its reaction clock"
    );
    let contact = w
        .find_contact(Side::Blue, t)
        .expect("actual onboard native visual target");
    let mount = w
        .entity(d)
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == "us_essm_i")
        .unwrap();
    let old_sam = w.entity(d).unwrap().weapons[mount].ammo;
    assert!(operations::sam_reaction_ready(
        &w,
        d,
        a,
        &w.db.weapons["us_essm_i"]
    ));
    assert_eq!(aa::fire_at(&mut w, d, a, mount, 1).unwrap(), 1);
    assert_eq!(w.entity(d).unwrap().weapons[mount].ammo, old_sam - 1);
    let packet = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .shots
        .values()
        .find(|p| p.permit.shooter == d && p.permit.target == a)
        .unwrap()
        .clone();
    assert_eq!(packet.identity,"aaw:3:us_essm_i:1:4","derive the normal scenario seed from the actual stable source packet, never rewrite a terminal result");
    assert!(packet
        .members
        .iter()
        .all(|m| w.entity(*m).is_some_and(|e| e.alive)));
    ac::surface_attack(&mut w, a, contact, "us_mk82", 6, ac::Delivery::Glide, "").unwrap();
    assert_eq!(stock(&w, a), 6);
    run(&mut w, 92);
    assert!(matches!(
        w.rules.aircombat.as_ref().unwrap().surface.passes[&a].phase,
        ac::surface::Phase::AwaitSam
    ));
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for state in [&mut w, &mut restored] {
        while !state
            .rules
            .aaw
            .as_ref()
            .unwrap()
            .outcomes
            .contains_key(&packet.identity)
            && state.tick < 125
        {
            assert_eq!(stock(state, a), 6);
            harpoon_core::sim::tick(state, &[]);
        }
        let out = state
            .rules
            .aaw
            .as_ref()
            .unwrap()
            .outcomes
            .get(&packet.identity)
            .expect("bounded real source projectile terminal must occur");
        assert_eq!(out.d100, 100);
        assert!(!out.hit);
        assert!(state.entity(a).unwrap().alive);
        assert!(state.events.iter().any(
            |e| matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="8.1.1.packet_terminal")
        ));
        let epoch = state.rules.aircombat.as_ref().unwrap().surface.scan_epoch;
        assert!(
            matches!(state.rules.aircombat.as_ref().unwrap().surface.passes[&a].phase,ac::surface::Phase::AwaitGuns{after_scan,completed_scan:None}if after_scan==epoch),
            "SAM clears after this tick gun scan: no same-tick release or second FCS sweep"
        );
        assert_eq!(stock(state, a), 6);
        assert!(packet
            .members
            .iter()
            .all(|m| !state.entity(*m).is_some_and(|e| e.alive)));
        harpoon_core::sim::tick(state, &[]);
        assert_eq!(
            state.rules.aircombat.as_ref().unwrap().surface.scan_epoch,
            epoch + 1,
            "exactly one next normal FCS scan"
        );
        assert!(state.entity(a).unwrap().pos.range_to(Vec2::zero()) > 0.1);
        assert_eq!(stock(state,a),6,"actual own position has flown beyond the original observed aim; waiting SAM cannot buy a free target/aircraft teleport");
        let p = &state.rules.aircombat.as_ref().unwrap().surface.passes[&a];
        assert!(p.pending.as_ref().is_some_and(|r| r.contains("geometry")));
        assert!(matches!(p.phase, ac::surface::Phase::AwaitGuns { .. }));
        assert!(!state.events.iter().any(
            |e| matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="9.6.source_bomb_attack")
        ));
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

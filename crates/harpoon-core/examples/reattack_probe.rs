use harpoon_core::{load_scenario, Command, ContactId, Database, EventKind, Scenario, Side, Vec2};
use serde_json::json;
fn main() {
    let mut seed: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    if seed == 0 {
        // replicate the test predicate with the true die semantics
        for cand in 1..=2_000_000u64 {
            let attack0 = {
                let mut r = harpoon_core::Rng::new(cand).derive(harpoon_core::Rng::hash_tag(0, 3, "10.2.5.attack:0"));
                1 + (r.next_f64() * 100.) as u8
            };
            let plc0 = {
                let mut r = harpoon_core::Rng::new(cand).derive(harpoon_core::Rng::hash_tag(0, 0, "10.placement:3:0"));
                1 + (r.next_f64() * 10.) as u8
            };
            if plc0 > 7 { continue; }
            if attack0 <= 80 { continue; }
            let plc1 = {
                let mut r = harpoon_core::Rng::new(cand).derive(harpoon_core::Rng::hash_tag(0, 0, "10.placement:3:1"));
                1 + (r.next_f64() * 10.) as u8
            };
            if plc1 > 7 { continue; }
            let attack1 = {
                let mut r = harpoon_core::Rng::new(cand).derive(harpoon_core::Rng::hash_tag(0, 3, "10.2.5.attack:1"));
                1 + (r.next_f64() * 100.) as u8
            };
            if attack1 > 80 { continue; }
            seed = cand;
            println!("probe chose seed {cand}: attack0={attack0} plc1={plc1} attack1={attack1}");
            break;
        }
        if seed == 0 { println!("no seed found"); return; }
    }
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"torpedo-reattack","name":"probe","duration_s":3000,"seed":seed,
        "context":{"rules_profile":"hv2026-rts-v4","torpedo_2026":{"source":"probe",
        "environment":{"source":"probe","layer_m":null,"reverberant":false},
        "targets":{"cn_093":{"source":"probe","size":2,"noise":2,"anechoic":true,"foil_borne":false}},
        "platforms":{"us_virginia":{"source":"probe","wire_capacity":2,"reload_parallel":2,
            "tubes":[{"bearing_deg":0,"loaded":"us_mk48"},{"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{"us_mk48":2}}},
        "orders":{"SHOOTER:us_mk48":{"source":"probe","speed_index":0,"depth_m":50,
            "enable_run_nm":0,"bearing_deg":null,"cut_wire":true,"spread_deg":[],
            "solution":{"quality":"Good","source":"probe"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,0.5],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    let mut w = load_scenario(sc, db).unwrap();
    let s = w.entities.values().find(|e| e.name == "SHOOTER").unwrap().id;
    let t = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    w.entity_mut(t).unwrap().weapons.clear();
    w.sides[0].contacts.insert(
        ContactId(99),
        harpoon_core::world::Contact {
            id: ContactId(99), about: t, category: harpoon_core::types::PlatformKind::Sub,
            class_name: None, estimate: Vec2::new(0., 0.5), est_course_deg: 0., est_speed_kts: 0.,
            bearing_only: false, quality: 1., last_update_tick: 0, first_tick: 0, stale: false,
            sensors: vec![], offset: Vec2::zero(), announced: true,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert("0:2".into(), harpoon_core::types::PlatformKind::Sub);
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, Command::Fire { unit: s, contact: ContactId(99), weapon: "us_mk48".into(), rounds: 1 })]);
    let torp = w.entities.values().find(|e| e.kind == harpoon_core::types::PlatformKind::Torpedo).map(|e| e.id);
    println!("seed={seed} torpedo={torp:?}");
    let mut ticks = 0;
    for _ in 0..3000 {
        if w.over { break; }
        harpoon_core::sim::tick(&mut w, &[]);
        ticks = w.tick;
        if let Some(st) = w.rules.torpedoes.as_ref() {
            if let Some(f) = st.flights.get(&harpoon_core::EntityId(3)) {
                println!("T{} phase={:?} run={:.3} fuel={:.3} pass={} plc={:?}",
                    w.tick, std::mem::discriminant(&f.phase), f.run_nm, f.fuel, f.pass,
                    f.placement.as_ref().map(|p| (p.placed, p.tick)));
            } else { println!("T{} flight gone", w.tick); break; }
        }
        if w.tick > 220 { break; }
    }
    for e in w.events.iter() {
        if let EventKind::RulesEvent { rule, detail, .. } = &e.kind {
            if rule.contains("placement") || rule.contains("terminal") || rule.contains("runout") || rule.contains("circle") || rule.contains("10.2.7") {
                println!("T{ticks} {rule} {detail}");
            }
        }
    }
    println!("over={}", w.over);
}

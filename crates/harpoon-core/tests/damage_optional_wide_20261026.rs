//! Wide-batch source candidates. No Cargo run by the author.
//! Direct critical/hit calls are explicitly adjudicated consumer boundaries.
//! Clock jumps isolate source-hour repair checks; they are not full sorties.
use harpoon_core::rules::{expanded_damage2026 as ex, repairs2026 as repairs, damage_runtime as dr};
use harpoon_core::{Database,EntityId,Scenario,World,Side,EventKind};
use serde_json::{json,Value};

fn id(w:&World,name:&str)->EntityId {w.entities.values().find(|e|e.name==name).unwrap().id}
fn source_context()->ex::CriticalContext {ex::CriticalContext {source:"scenario_assumption: adjudicated native critical consumer, not weapon hit claim".into(),dp:0.,penetrated:true,underwater:false,airburst:false,weapon:None}}
fn repair_order(w:&mut World,unit:EntityId,action:&str) {repairs::control(w,unit,action).unwrap().unwrap();}
fn service_fixture(seed:u64)->(World,EntityId) {
    let mut db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
    db.platforms.get_mut("us_ford").unwrap().sensors.clear();
    let sc:Scenario=serde_json::from_value(json!({"id":"wide-serviceability","name":"scenario_assumption: source failure and repair consumers","seed":seed,"duration_s":300000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","expanded_damage_2026":{"source":"scenario_assumption: explicitly selected14.7 laboratory system","optional":{"serviceability":true},"units":{"LAB":{"source":"scenario_assumption: one physical plant identity and first-rank repair table","rank":"first","repair_base":"at_sea","crew":100,"components":{"plant":{"source":"scenario_assumption: single explicit engineering plant","kind":"engineering"}}}}}},
        "units":[{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"speed_kts":0,"fire_on":false}]})).unwrap();
    let w=harpoon_core::load_scenario(sc,db).unwrap();let unit=id(&w,"LAB");(w,unit)
}
fn failed_service()->(World,EntityId) {
    (1..=4096).find_map(|seed|{let (w,unit)=service_fixture(seed);w.rules.expanded_damage.as_ref().unwrap().units.get(&unit)
        .is_some_and(|u|u.service_checks.values().any(|c|c.failed)).then_some((w,unit))}).expect("natural declared first-rank failure")
}
fn aircraft_fixture(seed:u64)->(World,EntityId) {
    let mut db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
    db.platforms.get_mut("us_f18e").unwrap().sensors.clear();
    let sc:Scenario=serde_json::from_value(json!({"id":"wide-aircraft-repair","name":"scenario_assumption: two adjudicated aircraft hits and source repair clocks","seed":seed,"duration_s":300000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","expanded_damage_2026":{"source":"scenario_assumption: selected14.9 detailed damage and land repair","optional":{"aircraft_damage":"immediate_detailed"},"units":{"AIR":{"source":"scenario_assumption: first-rank land maintenance and declared600kt cruise","rank":"first","land_based_aircraft":true,"cruise_speed_kts":600,"crew":2}}}},
        "units":[{"side":"Blue","class":"us_f18e","name":"AIR","pos":[0,0],"speed_kts":600,"altitude_m":4000,"fire_on":false}]})).unwrap();
    let w=harpoon_core::load_scenario(sc,db).unwrap();let unit=id(&w,"AIR");(w,unit)
}

#[test]
fn w01_manual_breakdown_repair_cannot_bypass_renewed_combat_and_actual_fire() {
    let (mut w,unit)=failed_service();
    repair_order(&mut w,unit,"begin_repairs");
    w.tick=6*3600;
    ex::source_critical(&mut w,unit,"wide-renewed-fire","fire",source_context()).unwrap();
    assert!(w.rules.casualties.get(&unit).is_some_and(|cs|!cs.is_empty()));
    repairs::step(&mut w);
    assert!(w.rules.repairs.as_ref().unwrap().units[&unit].pending.is_some());
    let before=serde_json::to_vec(&w).unwrap();
    assert!(ex::control(&mut w,unit,"serviceability_repair").unwrap().is_err(),
        "manual14.7 repair must respect the same real non-battle gate as automatic repair");
    assert_eq!(serde_json::to_vec(&w).unwrap(),before,"failed repair gate never rolls or restores a system");
}

#[test]
fn w02_real_failed_system_has_one_source_checkpoint_and_no_reroll() {
    let (mut w,unit)=failed_service();repair_order(&mut w,unit,"begin_repairs");
    w.tick=6*3600;
    ex::control(&mut w,unit,"serviceability_repair").unwrap().unwrap();
    let checks=&w.rules.expanded_damage.as_ref().unwrap().units[&unit].service_checks;
    assert!(checks.values().filter(|c|c.failed).all(|c|c.checks.contains_key(&6)));
    let before=serde_json::to_vec(&w).unwrap();
    ex::control(&mut w,unit,"serviceability_repair").unwrap().unwrap();
    assert_eq!(serde_json::to_vec(&w).unwrap(),before);
}

#[test]
fn w03_a_new_native_aircraft_hit_cannot_reuse_an_old_repaired_airframe_clock() {
    let (mut w,unit)=(1..=2048).find_map(|seed|{
        let (mut w,unit)=aircraft_fixture(seed);
        assert!(ex::aircraft_hit(&mut w,unit,"wide-first-hit"));
        if w.entity(unit).unwrap().hp<=0. {return None;}
        repair_order(&mut w,unit,"begin_repairs");w.tick=6*3600;
        ex::control(&mut w,unit,"serviceability_repair").unwrap().unwrap();
        if !w.rules.expanded_damage.as_ref().unwrap().units[&unit].aircraft.as_ref().unwrap().repaired {return None;}
        w.tick+=1;assert!(ex::aircraft_hit(&mut w,unit,"wide-second-hit"));
        (w.entity(unit).unwrap().hp>0.).then_some((w,unit))
    }).expect("original detailed damage + real successful repair + surviving second hit");
    let new_hit_at=w.tick;
    assert!(w.rules.repairs.as_ref().unwrap().units[&unit].pending.is_some(),
        "a new physical aircraft casualty requires a new explicit post-battle declaration");
    assert!(!w.rules.expanded_damage.as_ref().unwrap().units[&unit].aircraft.as_ref().unwrap().repaired);
    repair_order(&mut w,unit,"begin_repairs");
    assert_eq!(w.rules.repairs.as_ref().unwrap().units[&unit].session.as_ref().unwrap().began_at,new_hit_at);
    ex::control(&mut w,unit,"serviceability_repair").unwrap().unwrap();
    assert!(w.rules.expanded_damage.as_ref().unwrap().units[&unit].aircraft_repairs.is_empty(),
        "old6h roll must be archived rather than reused for a fresh casualty");
    let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    w.tick=new_hit_at+6*3600;restored.tick=w.tick;
    ex::control(&mut w,unit,"serviceability_repair").unwrap().unwrap();
    ex::control(&mut restored,unit,"serviceability_repair").unwrap().unwrap();
    assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap());
}

#[test]
fn w04_actual_fuel_exhaustion_dispatches_one_real_aircraft_survivor_group() {
    let mut db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
    db.platforms.get_mut("us_ford").unwrap().sensors.clear();
    db.platforms.get_mut("us_f18e").unwrap().sensors.clear();
    db.platforms.get_mut("us_f18e").unwrap().weapons.clear();
    let cruise=harpoon_core::rules::aviation2026::source_speed("us_f18e",4000.,harpoon_core::rules::aviation2026::Throttle::Cruise).unwrap();
    let sc:Scenario=serde_json::from_value(json!({"id":"wide-natural-ditch","name":"scenario_assumption: declared low remaining fuel over open sea","seed":20261026,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "aviation_2026":{"source":"scenario_assumption: bare aircraft and known remaining fuel, not a new performance value","parking":{},"loads":{"us_f18e":{"payload_kg":0,"tanks":[],"source":"scenario_assumption: bare aircraft"}},"initial_fuel_nm":{"AIR":0.01}},
            "expanded_damage_2026":{"source":"scenario_assumption: SAR option and explicit two-person laboratory complement","optional":{"search_and_rescue":true},"units":{"AIR":{"source":"scenario_assumption: two crew in this aircraft","crew":2}}}},
        "units":[{"side":"Blue","class":"us_ford","name":"LOOK","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"us_f18e","name":"AIR","pos":[10,0],"speed_kts":cruise,"altitude_m":4000,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"DUMMY","pos":[2000,0],"depth_m":50,"speed_kts":0,"fire_on":false}]})).unwrap();
    let mut w=harpoon_core::load_scenario(sc,db).unwrap();let air=id(&w,"AIR");
    harpoon_core::sim::tick(&mut w,&[]);
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::Ditched{unit,..}if *unit==air)),"actual source fuel exhaustion must occur");
    let groups=&w.rules.expanded_damage.as_ref().unwrap().survivors;
    assert_eq!(groups.len(),1,"native crash, not an authored survivor record, feeds14.8");
    assert_eq!(groups.values().next().unwrap().lost_unit,air);
    assert_eq!(groups.values().next().unwrap().count,2);
    assert!(ex::survivor_view(&w,id(&w,"LOOK")).is_empty(),"a loss notification alone is not discovery");
    harpoon_core::rules::operations::crash_aircraft(&mut w,air);
    assert_eq!(w.rules.expanded_damage.as_ref().unwrap().survivors.len(),1);
}

#[test]
fn w05_actual_belowdecks_magazine_explosion_uses_the_catastrophic_survival_column() {
    let (mut w,unit)=(1..=2048).find_map(|seed|{
        let mut db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
        let mut mount=db.platforms["us_virginia"].weapons.iter().find(|m|m.weapon=="us_mk48").unwrap().clone();
        mount.mounts=1;mount.ammo=4;
        db.platforms.get_mut("us_ford").unwrap().weapons=vec![mount];
        db.platforms.get_mut("us_ford").unwrap().sensors.clear();
        let sc:Scenario=serde_json::from_value(json!({"id":"wide-magazine-survival","name":"scenario_assumption: exact four-round laboratory magazine","seed":seed,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},
            "context":{"rules_profile":"hv2026-rts-v4","expanded_damage_2026":{"source":"scenario_assumption: exact magazine/complement declaration, not a Ford fit","optional":{"search_and_rescue":true},"units":{"LAB":{"source":"scenario_assumption: one hundred crew and explicit loaded magazine","crew":100,
                "components":{"mount":{"source":"scenario_assumption: one physical belowdecks loaded mount","kind":"weapon_mount","binding":"us_mk48","mount_index":0,"physical":0,"ammunition":4,"location":"belowdecks"}}}}}},
            "units":[{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"speed_kts":0,"fire_on":false}]})).unwrap();
        let mut w=harpoon_core::load_scenario(sc,db).unwrap();let unit=id(&w,"LAB");
        ex::source_critical(&mut w,unit,"wide-actual-magazine","weapon",source_context()).unwrap();
        (w.entity(unit).unwrap().hp<=0.).then_some((w,unit))
    }).expect("original mount-ammunition D10 causes a true source explosion");
    assert!(w.rules.expanded_dice[&unit].iter().any(|d|d["purpose"]=="mount_ammunition"&&d["roll"]==10));
    harpoon_core::damage::sink_by(&mut w,unit,Side::Red);
    let state=w.rules.expanded_damage.as_ref().unwrap();let g=state.survivors.values().next().unwrap();
    assert_eq!(g.count,u32::from(g.casualty_d6.unwrap()),"D6 percent of100 after actual magazine explosion, not D6*10percent");
    assert_eq!(g.source_crew,100);
    let restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap());
}

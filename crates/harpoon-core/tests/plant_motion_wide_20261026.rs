//! Wide-batch source candidates. No Cargo run by the author.
//! Direct critical/hit calls are explicitly adjudicated consumer boundaries.
//! Clock jumps isolate source-hour repair checks; they are not full sorties.
use harpoon_core::rules::{expanded_damage2026 as ex, repairs2026 as repairs};
use harpoon_core::{Database,EntityId,Scenario,World,Side,EventKind};
use serde_json::{json};

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
#[allow(dead_code)]
fn failed_service()->(World,EntityId) {
    (1..=4096).find_map(|seed|{let (w,unit)=service_fixture(seed);w.rules.expanded_damage.as_ref().unwrap().units.get(&unit)
        .is_some_and(|u|u.service_checks.values().any(|c|c.failed)).then_some((w,unit))}).expect("natural declared first-rank failure")
}

#[test]
fn w06_real_failed_plant_caps_actual_propulsion_and_source_repair_restores_the_retained_order() {
    let (mut w,unit)=(1..=4096).find_map(|seed|{
        let (w,unit)=service_fixture(seed);
        if !w.rules.expanded_damage.as_ref().unwrap().units.get(&unit).is_some_and(|u|u.service_checks.values().any(|c|c.failed)){return None;}
        let mut preview:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        // Match the real180-second propulsion segment below. Repair dice are
        // keyed to the original due time, so aT0 preview selects another die.
        preview.tick=180;
        repair_order(&mut preview,unit,"begin_repairs");preview.tick=180+21600;
        ex::control(&mut preview,unit,"serviceability_repair").unwrap().unwrap();
        ex::engineering_available(&preview,unit).unwrap().then_some((w,unit))
    }).expect("original failure and original successful6h repair seed");
    assert!(!ex::engineering_available(&w,unit).unwrap());
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,harpoon_core::Command::SetSpeed{unit,speed_kts:30.})]);
    for _ in 0..89 {harpoon_core::sim::tick(&mut w,&[]);}
    let mut restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..90 {harpoon_core::sim::tick(&mut w,&[]);harpoon_core::sim::tick(&mut restored,&[]);}
    assert_eq!(w.entity(unit).unwrap().speed_kts,0.,"true14.7 plant failure is consumed by propulsion, not only a private disabled label");
    assert_eq!(w.entity(unit).unwrap().ordered_speed_kts,30.,"equipment failure does not erase the player's standing order");
    assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap());
    repair_order(&mut w,unit,"begin_repairs");let began=w.tick;w.tick=began+21600;
    ex::control(&mut w,unit,"serviceability_repair").unwrap().unwrap();
    assert!(ex::engineering_available(&w,unit).unwrap());
    harpoon_core::sim::tick(&mut w,&[]);
    assert!(w.entity(unit).unwrap().speed_kts>0.,"the real repaired component releases the actual propulsion consumer");
}

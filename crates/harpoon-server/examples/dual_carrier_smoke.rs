//! Deterministic exercise integration check; not the human UI recording.
#[path = "../src/doctrine.rs"]
mod doctrine;
use harpoon_core::{observe, load_scenario, scenario_from_json, Database, Side, Command, AirMission};

fn run() -> harpoon_core::World {
    let db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc=scenario_from_json(include_str!("../../../scenarios/dual_carrier_denial.json")).unwrap();
    let mut world=load_scenario(sc,db).unwrap();
    world.event_cap=200_000;
    assert_eq!(world.entities.len(),34);
    assert_eq!(observe(&world,Side::Blue,0,0).units.len(),18);
    assert_eq!(observe(&world,Side::Red,0,0).units.len(),16);
    assert!(observe(&world,Side::Blue,0,0).contacts.is_empty());
    for unit in world.entities.values().filter(|u| u.name.starts_with("CV")) {
        let deck=unit.deck.as_ref().unwrap();
        assert!(deck.ready.iter().all(|(c,_)| c.starts_with("cn_") || c.starts_with("us_")));
    }
    let mut initial=Vec::new();
    for u in observe(&world,Side::Blue,0,0).units {
        if u.kind==harpoon_core::types::PlatformKind::Ship {initial.push((Side::Blue,Command::SetWeaponsFree{unit:u.id,free:true}));}
        if u.name.starts_with("CV-") {
            initial.push((Side::Blue,Command::LaunchAircraft{unit:u.id,class:"cn_j15".into(),count:2,mission:AirMission::Cap{station:u.pos,radius_nm:25.0}}));
            initial.push((Side::Blue,Command::LaunchAircraft{unit:u.id,class:"cn_z18f".into(),count:2,mission:AirMission::Asw{area:u.pos,radius_nm:12.0}}));
        }
    }
    while !world.over && world.tick<7200 {
        let obs=observe(&world,Side::Red,world.event_seq,0);
        let mut commands=doctrine::orders(&obs);
        if world.tick==0 {commands.extend(initial.clone());}
        harpoon_core::sim::tick(&mut world,&commands);
    }
    assert!(world.over,"mission must settle by the deadline");
    assert!(world.result.as_ref().unwrap().reason.starts_with("mission_"));
    assert!(world.command_log.iter().any(|c| c.side==Side::Red && matches!(c.command,Command::LaunchAircraft{..})));
    assert!(world.command_log.iter().any(|c| c.side==Side::Red && matches!(&c.command,Command::LaunchAircraft{class,..} if class=="us_f18e_asm")));
    world
}
fn main() {
    let a=run();let b=run();
    let serialized=serde_json::to_string(&a).unwrap();
    assert_eq!(serialized,serde_json::to_string(&b).unwrap(),"deterministic exercise replay");
    let result=a.result.as_ref().unwrap();
    println!("DUAL CARRIER PASS: tick={} winner={:?} reason={} events={} commands={}",a.tick,result.winner,result.reason,a.event_seq,a.command_log.len());
    if let Some(output)=std::env::args().nth(1) {
        let path=std::path::Path::new(&output);std::fs::create_dir_all(path).unwrap();
        std::fs::write(path.join("smoke-final-world.json"),serialized).unwrap();
        std::fs::write(path.join("smoke-final-blue.json"),serde_json::to_vec_pretty(&observe(&a,Side::Blue,0,200_000)).unwrap()).unwrap();
        std::fs::write(path.join("smoke-summary.json"),serde_json::to_vec_pretty(&serde_json::json!({"mode":"headless integration, not manual play","deterministic":true,"tick":a.tick,"result":result,"events":a.event_seq,"commands":a.command_log.len()})).unwrap()).unwrap();
    }
}

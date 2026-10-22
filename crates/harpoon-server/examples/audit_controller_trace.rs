//! Read-only controller audit against archived side-only observations.
//! Never changes the archived battle or gives the controller referee truth.
#[allow(dead_code)]
#[path = "../src/ai.rs"] mod ai;
#[allow(dead_code)]
#[path = "../src/tactical.rs"] mod tactical;
use ai::CommandStaff;
use harpoon_core::{Database, Observation, Side, Command};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn main() {
    let args:Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4, "audit_controller_trace completed-save initial-save output-json");
    let final_save:Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let initial:Value = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    let payload=&final_save["payload"];
    let db:Database=serde_json::from_value(payload["world"]["db"].clone()).unwrap();
    let mut staff:tactical::TacticalStaff=serde_json::from_value(initial["payload"]["tactical"][1]["staff"].clone()).unwrap();
    staff.restore_database(db.clone());
    let frames:Vec<Observation>=serde_json::from_value(payload["recording"]["frames"]["Red"].clone()).unwrap();
    let mut rows=Vec::new(); let mut mismatches=Vec::new();let mut matched=0;
    let mut reasons:BTreeMap<String,usize>=BTreeMap::new();
    for obs in frames.iter().filter(|o|!o.over) {
        let commands=staff.decide(obs);
        let notes=staff.take_notes();
        let actual:Vec<Command>=payload["recording"]["inputs"].as_array().unwrap().iter()
            .filter(|i| i["tick"].as_i64()==Some(obs.tick))
            .flat_map(|i| i["commands"].as_array().unwrap()).filter(|c| c[0]=="Red")
            .map(|c| serde_json::from_value(c[1].clone()).unwrap()).collect();
        if serde_json::to_value(&commands).unwrap()==serde_json::to_value(&actual).unwrap() {matched+=1;}
        else {mismatches.push(json!({"tick":obs.tick,"planned":commands.len(),"recorded":actual.len()}));}
        let minds=serde_json::to_value(&staff).unwrap();
        for u in obs.units.iter().filter(|u|u.mission.as_deref()==Some("strike")) {
            let key=u.id.0.to_string();let m=&minds["air_minds"][&key];
            let cid=m["target"].as_u64();
            let target=cid.and_then(|id|obs.contacts.iter().find(|c|c.id.0 as u64==id));
            let range=u.weapons.iter().filter(|w|w.ammo>0).filter_map(|w|db.weapons.get(&w.weapon))
                .filter(|w|w.kind==harpoon_core::db::WeaponKind::Ascm).map(|w|w.range_nm).fold(0.0,f64::max);
            let reason=if range<=0.0 {"empty"} else if target.is_none() {"missing_track"}
                else if target.unwrap().stale {"stale"} else if target.unwrap().bearing_only {"bearing_only"}
                else if target.unwrap().quality<0.4 {"quality"}
                else if u.pos.range_to(target.unwrap().estimate)>range*0.95 {"range"}
                else if u.altitude_m>60.0 {"altitude"} else {"release_or_group"};
            *reasons.entry(reason.into()).or_default()+=1;
            rows.push(json!({"tick":obs.tick,"unit":u.id,"phase":m["phase"],"target":cid,
                "reason":reason,"range_nm":range,"altitude_m":u.altitude_m,"endurance_s":u.endurance_s,
                "target_distance":target.map(|c|u.pos.range_to(c.estimate)),"quality":target.map(|c|c.quality),
                "stale":target.map(|c|c.stale),"notes":notes.iter().filter(|n|n.unit==u.id).collect::<Vec<_>>(),
                "commands":commands.iter().filter(|c|match c {Command::SetAltitude{unit,..}|Command::Fire{unit,..}|Command::ReturnToBase{unit}=>*unit==u.id,_=>false}).collect::<Vec<_>>() }));
        }
    }
    let result=json!({"side":Side::Red,"observation_frames":frames.len(),"matched_command_windows":matched,
        "mismatch_count":mismatches.len(),"mismatches":mismatches,"blocking_samples":reasons,"strike_samples":rows});
    std::fs::write(&args[3],serde_json::to_vec(&result).unwrap()).unwrap();
    println!("{}",json!({"matched":matched,"mismatches":result["mismatch_count"],"blocking_samples":result["blocking_samples"]}));
}

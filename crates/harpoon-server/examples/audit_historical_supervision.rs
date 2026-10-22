//! Historical-input guard audit. Observations stay frozen; this is not a new
//! LLM match or a simulation of the outcomes of the amended commands.
#[allow(dead_code)]
#[path="../src/ai.rs"]mod ai;
#[allow(dead_code)]
#[path="../src/tactical.rs"]mod tactical;
use harpoon_core::{Database,Command,observe::Observation};
use serde::Deserialize;
#[derive(Deserialize)]struct Window {observation:Observation,commands:Vec<Command>}
fn main(){
 let a:Vec<_>=std::env::args().collect();let windows:Vec<Window>=serde_json::from_slice(&std::fs::read(&a[1]).unwrap()).unwrap();
 let mut db=Database::built_in();let mut paths:Vec<_>=std::fs::read_dir("database/volumes").unwrap().flatten().map(|e|e.path()).filter(|p|p.extension().is_some_and(|x|x=="json")&&!p.file_name().unwrap().to_string_lossy().starts_with('_')).collect();paths.sort();
 for p in paths {db=db.merged(&Database::from_json(&std::fs::read_to_string(p).unwrap()).unwrap());}
 let mut policy=tactical::supervision::Supervisor::default();let mut result=Vec::new();
 for window in windows {let o=&window.observation;let mut notes=policy.observe(o,&db);notes.extend(policy.review_current(o,&db));let (maintenance,review)=policy.maintenance(o,&db);notes.extend(review);
  let original=serde_json::to_value(&window.commands).unwrap();let (accepted,guard)=policy.guard(o,&db,window.commands);notes.extend(guard);
  result.push(serde_json::json!({"tick":o.tick,"original":original,"guarded":accepted,"maintenance":maintenance,"notes":notes,"supervision":policy.summary(o)}));
 }
 let output=serde_json::json!({"kind":"historical-input guard audit","new_model_calls":0,"counterfactual_outcomes_simulated":false,"notes":"Frozen historical observations; transport validation and altered battle outcomes are not evaluated here","windows":result});
 std::fs::write(&a[2],serde_json::to_vec(&output).unwrap()).unwrap();println!("{} historical windows audited; no new model match",output["windows"].as_array().unwrap().len());
}

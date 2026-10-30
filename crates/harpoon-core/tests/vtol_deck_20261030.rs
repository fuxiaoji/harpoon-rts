//! 14.5 forward flight deck critical: source fire input is explicitly adjudicated.
use harpoon_core::{World,Scenario,Database,Side,EntityId,Command,Vec2};
use harpoon_core::db::{WeaponKind,Guidance};
use harpoon_core::rules::{damage_runtime as d,expanded_damage2026 as e};
use harpoon_core::commands::AirMission;
use serde_json::json;
fn carrier(w:&World)->EntityId {w.entities.values().find(|e|e.name=="CV").unwrap().id}
fn fixture(seed:u64)->World {
 let mut db=Database::from_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../database/consolidated.json"))).unwrap();
 // Explicit laboratory CAP load: retain only source air-to-air mounts, no
 // unqualified bomb readiness classification or real carrier fit claim.
 let keep:Vec<_>=db.platforms["us_f35b"].weapons.iter().filter(|m|db.weapons.get(&m.weapon).is_some_and(|d|d.kind!=WeaponKind::Bomb)).cloned().collect();
 db.platforms.get_mut("us_f35b").unwrap().weapons=keep;
 let sc:Scenario=serde_json::from_value(json!({"id":"vtol-deck","name":"scenario_assumption: experimental F35B deck and zone, no actual Ford fit","seed":seed,"duration_s":1800,"weather":{"visibility_nm":10,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","flight_environment":{"wind_from_deg":0,"wind_kts":0},"radar_environment":{"sea_state":0,"precipitation":"None"},"aviation_2026":{"source":"scenario_assumption: exercise loads","parking":{},"loads":{"us_f35b":{"payload_kg":1000,"tanks":[],"source":"scenario_assumption: experiment"},"us_f18e":{"payload_kg":1000,"tanks":[],"source":"scenario_assumption: experiment"}}},"air_wings":{"CV":[{"class":"us_f35b","count":1},{"class":"us_f18e","count":1}]},"expanded_damage_2026":{"source":"scenario_assumption: test deck","units":{"CV":{"source":"scenario_assumption: explicit deck","vtol_launch":{"source":"scenario_assumption: experimental aft spot and cycle; not real platform fit","zone":"aft","pads":1,"duration_s":30}}}}},"units":[{"side":"Blue","class":"us_ford","name":"CV","pos":[0,0],"speed_kts":0,"fire_on":false},{"side":"Red","class":"cn_093","name":"DISTANT","pos":[200,200],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
 harpoon_core::load_scenario(sc,db).unwrap()
}
fn launch(id:EntityId,class:&str)->Command {Command::LaunchAircraft{unit:id,class:class.into(),count:1,mission:AirMission::Cap{station:Vec2::new(0.,0.),radius_nm:5.,follow:true},profile:None}}
#[test]
fn forward_damage_vtol_explicit_undamaged_zone_but_no_power_bypass() {
 for seed in 1..=32 {let mut w=fixture(seed);let id=carrier(&w);let mut weapon=w.db.weapons["us_mk82"].clone();weapon.kind=WeaponKind::Bomb;weapon.guidance=Guidance::BallisticGun;weapon.damage=30.;
  weapon.rule_data=Some(d::WeaponDamage{ship_dp:Some(30.),penetration_cm:Some(100.),source:"scenario_assumption: adjudicated deck damage laboratory".into(),..Default::default()});
  d::queue(&mut w,id,&weapon,false,Some(90.),Some(Side::Red),Some("deck-test".into()));d::finish(&mut w);
  if !w.entity(id).is_some_and(|c|c.alive && c.ext.get("damage.deck_launch")==Some(&1.) && c.ext.get("damage.power_lost").is_none()) {continue;}
  assert!(!e::flight_launch_operational(&w,id,"us_f18e"));assert!(e::flight_launch_operational(&w,id,"us_f35b"));
  assert!(harpoon_core::air::launch_aircraft(&mut w,id,"us_f18e",1,&AirMission::Cap{station:Vec2::new(0.,0.),radius_nm:5.,follow:true}).is_err());
  let mut probe=w.clone();let direct=harpoon_core::air::launch_aircraft(&mut probe,id,"us_f35b",1,&AirMission::Cap{station:Vec2::new(0.,0.),radius_nm:5.,follow:true});
  let cmd=launch(id,"us_f35b");harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]);
  let rejected:Vec<_>=w.events.iter().filter_map(|ev|match &ev.kind {harpoon_core::EventKind::DeckBusy{reason,..}=>Some(reason.clone()),_=>None}).collect();
  assert!(w.rules.operations.as_ref().unwrap().launches.iter().any(|j|j.class=="us_f35b"), "direct={direct:?}, deckbusy={rejected:?}, aviation={:?}, deck={:?}",w.rules.aviation,w.entity(id).unwrap().deck);
  for _ in 0..500 {harpoon_core::sim::tick(&mut w,&[]); if w.entities.values().any(|x|x.class=="us_f35b") {return;}}
  panic!("qualified queued VTOL did not actually launch");
 } panic!("fixture precondition: no forward deck critical without electrical loss in 32 seeds");
}

// Append to vtol_deck_20261030.rs after 83. Explicit pre-adjudicated
// source critical inputs, not a claimed natural weapon hit or ISR chain.
#[test]
fn powered_down_vtol_rejected_by_public_launch_command() {
    use harpoon_core::rules::expanded_damage2026::{source_critical,CriticalContext};
    let ctx=|| CriticalContext{source:"scenario_assumption: adjudicated engineering critical, not natural attack".into(),dp:0.,penetrated:true,underwater:false,airburst:false,weapon:None};
    for seed in 1..=32 {
        let mut w=fixture(seed); let id=carrier(&w);
        // Distinct actual engineering critical identities; source D6 remains
        // the engine's real die and may leave power intact.
        for k in 0..3 {
            source_critical(&mut w,id,&format!("power-control-{k}"),"engineering_no_fire",ctx()).unwrap();
            if w.entity(id).unwrap().ext.get("damage.power_lost").is_some_and(|v|*v>w.tick as f64) { break; }
        }
        if !w.entity(id).unwrap().ext.get("damage.power_lost").is_some_and(|v|*v>w.tick as f64) {continue;}
        assert!(!e::flight_launch_operational(&w,id,"us_f35b"));
        let before=w.entity(id).unwrap().deck.as_ref().unwrap().ready.clone();
        let cmd=launch(id,"us_f35b");
        harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]);
        assert_eq!(w.entity(id).unwrap().deck.as_ref().unwrap().ready,before);
        assert!(!w.rules.operations.as_ref().unwrap().launches.iter().any(|j|j.class=="us_f35b"));
        return;
    }
    panic!("fixture precondition: no electrical loss in bounded real engineering dice");
}

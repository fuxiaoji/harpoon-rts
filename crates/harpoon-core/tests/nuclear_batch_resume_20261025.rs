//! Explicit laboratory delivery and source-table fixtures, not real nuclear fits.
use harpoon_core::{rules::nuclear2026 as n, Database, EntityId, Scenario, Side, World};
use serde_json::json;

fn fixture(seed:u64, authorized:bool)->(World,EntityId,EntityId){
    let db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let sc:Scenario=serde_json::from_value(json!({"id":"nuclear-source-lab","name":"scenario_assumption: nuclear lifecycle laboratory","duration_s":10000,"seed":seed,
        "weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","nuclear_2026":{"source":"scenario_assumption: laboratory delivery, not a historical ESSM nuclear variant","units":{"LAB":{"us_essm_i":{"source":"scenario_assumption: laboratory-only delivery","source_variant":"scenario_assumption:lifecycle_lab","allowed_yields_kt":[1,5],"selected_yield_kt":1,"medium":"air","fuze":"free_fall","authorized":authorized}}}}},
        "units":[{"side":"Blue","class":"us_ford","name":"LAB","pos":[0,0],"speed_kts":0,"fire_on":false},{"side":"Red","class":"cn_052d","name":"TARGET","pos":[0,10],"speed_kts":0,"fire_on":false}]})).unwrap();
    let w=harpoon_core::load_scenario(sc,db).unwrap();
    let b=w.entities.values().find(|e|e.name=="LAB").unwrap().id;
    let t=w.entities.values().find(|e|e.name=="TARGET").unwrap().id;
    (w,b,t)
}
fn launch(w:&mut World,b:EntityId,t:EntityId)->EntityId{
    let def=w.db.weapons["us_essm_i"].clone();
    harpoon_core::weapons::launch(w,b,&def,Some(t),None).unwrap()
}
#[test]
fn every_discrete_yield_and_boundary_uses_the_printed_table(){
    assert_eq!(n::tables()["rows"].as_array().unwrap().len(),36);
    for row in n::tables()["rows"].as_array().unwrap(){
        let y=row[0].as_u64().unwrap() as u32;
        let [kill,_,damage]=n::radii(y).unwrap();
        assert_eq!(n::effect(y,n::Medium::Surface,harpoon_core::types::PlatformKind::Ship,true,false,kill).unwrap(),n::Effect::Destroyed);
        assert_eq!(n::effect(y,n::Medium::Surface,harpoon_core::types::PlatformKind::Ship,true,false,damage).unwrap(),n::Effect::SurfaceDamage);
        assert_eq!(n::effect(y,n::Medium::Surface,harpoon_core::types::PlatformKind::Ship,true,false,damage+1e-7).unwrap(),n::Effect::None);
    }
    assert!(n::radii(4).is_err());
    assert!(n::effect(1,n::Medium::Surface,harpoon_core::types::PlatformKind::Ship,true,false,f64::NAN).is_err());
}
#[test]
fn native_library_release_cannot_bypass_nuclear_authorization(){
    let (mut w,b,t)=fixture(1,false);let before=serde_json::to_vec(&w).unwrap();
    let def=w.db.weapons["us_essm_i"].clone();
    assert!(harpoon_core::weapons::launch(&mut w,b,&def,Some(t),None).is_none());
    assert_eq!(serde_json::to_vec(&w).unwrap(),before);
}
#[test]
fn native_release_snapshots_warhead_before_later_orders(){
    let (mut w,b,t)=fixture(2,true);let p=launch(&mut w,b,t);
    n::apply(&mut w,Side::Blue,b,&n::Action::SetYield{weapon:"us_essm_i".into(),yield_kt:5}).unwrap();
    n::observe_flight(&mut w,p);
    let state=serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
    assert_eq!(state["flights"][p.0.to_string()]["warhead"]["selected_yield_kt"],1);
}
#[test]
fn guided_miss_retains_one_delayed_burst_and_survives_world_roundtrip(){
    let (mut w,b,t)=fixture(3,true);let p=launch(&mut w,b,t);
    n::observe_flight(&mut w,p);assert!(n::removed(&mut w,p,n::Removal::GuidedMiss));
    let first=serde_json::to_vec(&w).unwrap();
    n::removed(&mut w,p,n::Removal::GuidedMiss);assert_eq!(serde_json::to_vec(&w).unwrap(),first);
    let mut copy:World=serde_json::from_slice(&first).unwrap();
    w.tick=29;copy.tick=29;n::step(&mut w);n::step(&mut copy);
    assert!(!w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,..}if rule=="14.1.8.detonation")));
    w.tick=30;copy.tick=30;n::step(&mut w);n::step(&mut copy);
    assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&copy).unwrap());
    assert_eq!(w.events.iter().filter(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,..}if rule=="14.1.8.detonation")).count(),1);
}
#[test]
fn target_assignment_does_not_claim_terminal_seeker_acquisition(){
    let (mut w,b,t)=fixture(4,true);
    w.rules.nuclear.as_mut().unwrap().fits.get_mut(&b).unwrap().get_mut("us_essm_i").unwrap().fuze=n::Fuze::TerminalSeeker;
    let p=launch(&mut w,b,t);
    w.entity_mut(p).unwrap().projectile.as_mut().unwrap().enabled=true;
    n::observe_flight(&mut w,p);
    let state=serde_json::to_value(w.rules.nuclear.as_ref().unwrap()).unwrap();
    assert_eq!(state["flights"][p.0.to_string()]["acquired"],false);
}
#[test]
fn source_unknown_facility_effect_never_becomes_an_invented_damage_value(){
    assert!(matches!(n::effect(1,n::Medium::Surface,harpoon_core::types::PlatformKind::Base,true,false,0.1).unwrap(),n::Effect::Pending(_)));
}
#[test]
fn native_sam_hit_with_failed_nuclear_function_does_not_apply_conventional_kill(){
    use harpoon_core::rules::aaw2026 as a;
    // A frozen source-example packet is an explicit terminal-input fixture.
    // It proves the real projectile consumer, not historical ESSM nuclear fit.
    let seed=(0..100000).find(|s|{
        let mut function=harpoon_core::Rng::new(*s).derive(harpoon_core::Rng::hash_tag(0,0,"14.1.8:3:function"));
        let mut terminal=harpoon_core::Rng::new(*s).derive(harpoon_core::Rng::hash_tag(0,1,"aaw:1:us_essm_i:2:3"));
        1+(function.next_f64()*10.) as u8==10 && 1+(terminal.next_f64()*100.) as u8<=35
    }).unwrap();
    let (mut w,b,t)=fixture(seed,true);
    w.entity_mut(t).unwrap().kind=harpoon_core::types::PlatformKind::Aircraft;
    w.entity_mut(t).unwrap().pos=harpoon_core::Vec2::new(0.,0.2);
    let p=launch(&mut w,b,t);assert_eq!(p.0,3);
    let attack=a::MissileAttack{ata:2.,maneuver_rating:0.,maneuvering:false,speed_kts:2003.,signature:2,combat_generation:4,automatic:false,target_is_missile:true,range_nm:30.,bearing:a::Bearing::Closing,terminal_maneuvers:false,sea_skimming:false,sea_capability:2,noe:false,manpads_or_salh:false,high_diving:false,land_clutter:false,radar_guided:true,countermeasures:None,hoj:false};
    let budget=a::intercept_budget(4,false,16000.,2003.,68.8,1650.,3.5,false,None).unwrap();
    a::commit(w.rules.aaw.as_mut().unwrap(),a::Permit{key:"scenario_assumption: frozen source packet".into(),channel:0,shooter:b,target:t,weapon:"us_essm_i".into(),tick:0,band:a::Band::Long1,budget,initial_course:0.,initial_range:30.,attack,missiles:1},&[p]).unwrap();
    harpoon_core::weapons::advance_projectiles(&mut w);
    assert!(w.rules.aaw.as_ref().unwrap().outcomes.values().next().unwrap().hit);
    n::step(&mut w);
    assert!(w.entity(t).unwrap().alive,"a failed nuclear D10 cannot inherit conventional SAM destruction");
    assert!(w.events.iter().any(|e|matches!(&e.kind,harpoon_core::EventKind::RulesEvent{rule,detail,..}if rule=="14.1.8.detonation"&&detail.contains("\"failed\":true"))));
}

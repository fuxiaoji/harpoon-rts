use harpoon_core::{Database,World,Side,Command,EntityId,ContactId,Vec2,EventKind};
use harpoon_core::types::PlatformKind;

fn scenario()->serde_json::Value {serde_json::json!({"id":"fictional-exercise-test","name":"Fictional exercise","duration_s":1000,"seed":9,
    "weather":{"visibility_nm":1,"jam":[0,0]},"context":{"island_exercise":{
        "locked_side":"Red","briefing_side":"Blue","briefing_targets":["RCV","RAIR"],
        "protected_targets":[{"name":"RCV","group":"carrier","points":60},{"name":"RAIR","group":"infrastructure","points":40}],
        "counter_targets":[{"name":"BCV","group":"counter","points":20}],"required_carrier_points":30,"required_infrastructure_points":20}},
    "units":[{"side":"Blue","class":"us_ford","name":"BCV","pos":[0,0]},
        {"side":"Red","class":"us_ford","name":"RCV","pos":[300,0]},
        {"side":"Red","class":"airfield","name":"RAIR","pos":[310,0]}]})}
fn world()->World {
    let db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    harpoon_core::load_scenario(harpoon_core::scenario_from_json(&scenario().to_string()).unwrap(),db).unwrap()
}
#[test]fn joint_groups_are_side_private_even_after_all_members_are_lost_and_restore_exactly() {
    let db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();let mut s=scenario();
    s["context"]["island_exercise"]["joint_groups"]=serde_json::json!([
        {"side":"Blue","name":"BLUE NAVAL","role":"naval","members":["BCV"]},
        {"side":"Red","name":"RED AVIATION","role":"shore_aviation","members":["RAIR"]}]);
    let mut w=harpoon_core::load_scenario(harpoon_core::scenario_from_json(&s.to_string()).unwrap(),db).unwrap();
    let blue=id(&w,"BCV");w.entity_mut(blue).unwrap().alive=false;
    for side in [Side::Blue,Side::Red] {let e=harpoon_core::observe(&w,side,0,0).exercise.unwrap();assert_eq!(e.joint_groups.len(),1);assert_eq!(e.joint_groups[0].side,side);}
    let restored:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();assert_eq!(serde_json::to_value(&restored).unwrap(),serde_json::to_value(&w).unwrap());
    assert!(serde_json::to_value(world()).unwrap()["context"]["island_exercise"].get("joint_groups").is_none());
}
#[test]fn malformed_joint_ownership_and_repeated_members_are_rejected() {
    let db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for members in [serde_json::json!(["RCV"]),serde_json::json!(["BCV","BCV"]),serde_json::json!([])] {
        let mut s=scenario();s["context"]["island_exercise"]["joint_groups"]=serde_json::json!([{"side":"Blue","name":"BAD","role":"naval","members":members}]);
        assert!(harpoon_core::load_scenario(harpoon_core::scenario_from_json(&s.to_string()).unwrap(),db.clone()).is_err());
    }
}
fn id(w:&World,n:&str)->EntityId {w.entities.values().find(|u|u.name==n).unwrap().id}

fn opening_scenario()->serde_json::Value {
    let mut s=scenario();
    s["context"]["air_wings"]=serde_json::json!({"BCV":[{"class":"us_f18e_asm","count":8}]});
    s["context"]["island_exercise"]["opening_flights"]=serde_json::json!([
        {"home":"BCV","class":"us_f18e_asm","count":4,"target":"RCV","progress":0.25,"altitude_m":50,"escort":false}]);
    s["units"].as_array_mut().unwrap().push(serde_json::json!({"side":"Blue","class":"cn_055","name":"BESC","pos":[200,0]}));
    s["context"]["island_exercise"]["opening_salvos"]=serde_json::json!([
        {"launcher":"BESC","weapon":"cn_yj18","count":4,"target":"RCV","progress":0.3}]);
    s
}
#[test]fn opening_snapshot_consumes_inventory_and_time_without_exposing_flight_tasks_to_red() {
    let db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut s=opening_scenario();
    let esc=db.platforms.get("cn_055").unwrap();
    let weapon=esc.weapons.iter().find(|m|db.weapons[&m.weapon].kind==harpoon_core::db::WeaponKind::Ascm).unwrap().weapon.clone();
    s["context"]["island_exercise"]["opening_salvos"][0]["weapon"]=serde_json::json!(weapon);
    let w=harpoon_core::load_scenario(harpoon_core::scenario_from_json(&s.to_string()).unwrap(),db.clone()).unwrap();
    let cv=id(&w,"BCV");assert_eq!(w.entity(cv).unwrap().deck.as_ref().unwrap().ready[0].1,4);
    let airborne:Vec<_>=w.entities.values().filter(|u|u.side==Side::Blue&&u.kind==PlatformKind::Aircraft).collect();assert_eq!(airborne.len(),4);
    for u in airborne {assert_eq!(u.altitude_m,50.0);assert!(u.pos.x>74.0&&u.pos.x<76.0);assert!(u.endurance_s.unwrap()<db.platforms["us_f18e_asm"].endurance_s);assert_eq!(u.waypoints.len(),1);assert!(!u.fire_on);}
    let missiles:Vec<_>=w.entities.values().filter(|u|u.kind==PlatformKind::Missile).collect();assert_eq!(missiles.len(),4);
    assert!(missiles.iter().all(|u|u.projectile.as_ref().unwrap().live_s>0.0));
    let mount=w.entity(id(&w,"BESC")).unwrap().weapons.iter().find(|m|m.weapon==weapon).unwrap();
    let fit=esc.weapons.iter().find(|m|m.weapon==weapon).unwrap();assert_eq!(mount.ammo,fit.ammo*fit.mounts as u16-4);
    assert_eq!(harpoon_core::observe(&w,Side::Blue,0,0).exercise.unwrap().opening_flights.len(),4);
    assert!(harpoon_core::observe(&w,Side::Red,0,0).exercise.unwrap().opening_flights.is_empty());
    assert!(harpoon_core::exercise::locked(&w,Side::Red));
    let loaded:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();assert_eq!(serde_json::to_value(&loaded).unwrap(),serde_json::to_value(&w).unwrap());
}
#[test]fn invalid_opening_inventory_target_progress_and_envelope_reject_the_new_world() {
    let db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for case in 0..5 {
        let mut s=opening_scenario();
        match case {
            0=>s["context"]["island_exercise"]["opening_flights"][0]["count"]=serde_json::json!(99),
            1=>s["context"]["island_exercise"]["opening_flights"][0]["progress"]=serde_json::json!(1.0),
            2=>s["context"]["island_exercise"]["opening_flights"][0]["target"]=serde_json::json!("BCV"),
            3=>s["context"]["island_exercise"]["opening_flights"][0]["altitude_m"]=serde_json::json!(-1),
            _=>s["units"][3]["pos"]=serde_json::json!([20000,0]),
        }
        assert!(harpoon_core::load_scenario(harpoon_core::scenario_from_json(&s.to_string()).unwrap(),db.clone()).is_err());
    }
}
fn contact(w:&mut World,kind:PlatformKind) {
    let target=id(w,"BCV");let c=ContactId(1);w.sides[1].next_contact=1;
    w.sides[1].contacts.insert(c,harpoon_core::world::Contact {id:c,about:target,category:kind,class_name:None,
        estimate:Vec2::zero(),est_course_deg:0.0,est_speed_kts:0.0,bearing_only:false,quality:0.5,
        last_update_tick:w.tick,first_tick:w.tick,stale:false,sensors:vec!["fixture".into()],offset:Vec2::zero(),announced:false});
}

#[test]fn lock_blocks_actual_orders_but_aircraft_contacts_do_not_unlock_it() {
    let mut w=world();let cv=id(&w,"RCV");let before=w.entity(cv).unwrap().ordered_speed_kts;
    harpoon_core::sim::tick(&mut w,&[(Side::Red,Command::SetSpeed {unit:cv,speed_kts:20.0})]);
    assert_eq!(w.entity(cv).unwrap().ordered_speed_kts,before);
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent {rule,..} if rule=="command_locked")));
    contact(&mut w,PlatformKind::Aircraft);harpoon_core::exercise::update_alert(&mut w);
    assert!(harpoon_core::exercise::locked(&w,Side::Red));
    contact(&mut w,PlatformKind::Missile);harpoon_core::exercise::update_alert(&mut w);
    assert!(!harpoon_core::exercise::locked(&w,Side::Red));
    let seq=w.event_seq;harpoon_core::exercise::update_alert(&mut w);assert_eq!(w.event_seq,seq);
    harpoon_core::sim::tick(&mut w,&[(Side::Red,Command::SetSpeed {unit:cv,speed_kts:20.0})]);
    assert_eq!(w.entity(cv).unwrap().ordered_speed_kts,20.0);
}

#[test]fn briefing_is_a_snapshot_and_enemy_damage_and_alert_state_stay_private() {
    let mut w=world();let cv=id(&w,"RCV");w.entity_mut(cv).unwrap().hp*=0.5;
    contact(&mut w,PlatformKind::Missile);harpoon_core::exercise::update_alert(&mut w);
    let blue=harpoon_core::observe(&w,Side::Blue,0,100);let red=harpoon_core::observe(&w,Side::Red,0,100);
    assert!(blue.exercise.as_ref().unwrap().alerted_at.is_none());
    assert_eq!(red.exercise.as_ref().unwrap().alerted_at,Some(0));
    assert!(blue.exercise.as_ref().unwrap().objectives.iter().find(|o|o.name=="RCV").unwrap().hp_fraction.is_none());
    assert_eq!(red.exercise.as_ref().unwrap().objectives.iter().find(|o|o.name=="RCV").unwrap().hp_fraction,Some(0.5));
    let old=blue.contacts[0].estimate;w.entity_mut(cv).unwrap().pos.x+=20.0;
    let after=harpoon_core::observe(&w,Side::Blue,0,100);assert_eq!(after.contacts[0].estimate,old);
    let value=serde_json::to_value(after).unwrap();
    assert!(value["contacts"].as_array().unwrap().iter().all(|c|c.get("about").is_none() && c.get("offset").is_none()));
    assert!(!value["events"].as_array().unwrap().iter().any(|e|e["rule"]=="alert"));
}

#[test]fn objective_categories_and_preservation_counterstrike_decide_the_result() {
    let mut w=world();assert!(harpoon_core::exercise::adjudicate(&w).is_none());w.tick=1000;
    assert_eq!(harpoon_core::exercise::adjudicate(&w).unwrap().0,Some(Side::Red));
    let cv=id(&w,"RCV");let base=id(&w,"RAIR");let blue=id(&w,"BCV");
    w.entity_mut(cv).unwrap().hp*=0.4;w.entity_mut(base).unwrap().hp*=0.4;
    let(b,r,c,i)=harpoon_core::exercise::scores(&w).unwrap();assert_eq!((b,r,c,i),(60.0,40.0,36.0,24.0));
    assert_eq!(harpoon_core::exercise::adjudicate(&w).unwrap().0,Some(Side::Blue));
    w.entity_mut(blue).unwrap().hp=0.0;
    assert_eq!(harpoon_core::exercise::adjudicate(&w).unwrap().0,Some(Side::Red));
    // Infrastructure alone never satisfies the carrier condition.
    w.entity_mut(cv).unwrap().hp=w.entity(cv).unwrap().hp_max;w.entity_mut(base).unwrap().hp=0.0;
    assert_eq!(harpoon_core::exercise::adjudicate(&w).unwrap().0,Some(Side::Red));
    let decoded:World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&decoded).unwrap(),serde_json::to_value(&w).unwrap());
}

#[test]fn malformed_objectives_are_rejected_without_guessing_targets() {
    let db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for change in 0..3 {
        let mut value=scenario();let c=&mut value["context"]["island_exercise"];
        match change {0=>c["briefing_targets"][0]=serde_json::json!("unknown"),1=>c["required_carrier_points"]=serde_json::json!(90),_=>c["protected_targets"][0]["group"]=serde_json::json!("counter")};
        assert!(harpoon_core::load_scenario(harpoon_core::scenario_from_json(&value.to_string()).unwrap(),db.clone()).is_err());
    }
}

#[test]fn groups_are_side_private_and_authored_air_preparation_preserves_inventory() {
    let db=Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();let mut s=scenario();
    s["context"]["island_exercise"]["naval_groups"]=serde_json::json!([
        {"side":"Blue","name":"B-GROUP","leader":"BCV","members":["BCV"]},
        {"side":"Red","name":"R-GROUP","leader":"RCV","members":["RCV"]}]);
    s["context"]["island_exercise"]["defender_air_preparation_s"]=serde_json::json!(360);
    let mut w=harpoon_core::load_scenario(harpoon_core::scenario_from_json(&s.to_string()).unwrap(),db).unwrap();
    let blue=harpoon_core::observe(&w,Side::Blue,0,0);let red=harpoon_core::observe(&w,Side::Red,0,0);
    assert_eq!(blue.exercise.unwrap().naval_groups[0].name,"B-GROUP");
    assert_eq!(red.exercise.unwrap().naval_groups[0].name,"R-GROUP");
    let r=id(&w,"RCV");let deck=w.entity(r).unwrap().deck.as_ref().unwrap();
    let total:u32=deck.ready.iter().map(|(_,n)|*n as u32).sum::<u32>()+deck.cycling.iter().map(|(_,n,_)|*n as u32).sum::<u32>();
    assert_eq!(deck.ready.iter().map(|(_,n)|*n as u32).sum::<u32>(),2);
    assert!(deck.cycling.iter().all(|(_,_,t)|*t==360));
    let class=deck.ready.iter().find(|(_,n)|*n>0).unwrap().0.clone();
    assert!(harpoon_core::air::launch_aircraft(&mut w,r,&class,3,&harpoon_core::commands::AirMission::Recon {point:Vec2::zero()}).is_err());
    w.tick=360;harpoon_core::air::update_air_ops(&mut w);
    let deck=w.entity(r).unwrap().deck.as_ref().unwrap();
    assert_eq!(deck.ready.iter().map(|(_,n)|*n as u32).sum::<u32>(),total);
    assert!(deck.cycling.is_empty());
    // A group may not smuggle another side's entity through a friendly plan.
    s["context"]["island_exercise"]["naval_groups"][0]["members"][0]=serde_json::json!("RCV");
    assert!(harpoon_core::load_scenario(harpoon_core::scenario_from_json(&s.to_string()).unwrap(),w.db.clone()).is_err());
}

#[test]fn actual_hostile_impact_unlocks_without_a_sensor_contact_and_deadline_is_scored() {
    let mut w=world();let cv=id(&w,"RCV");
    w.emit(EventKind::Impact {side:Side::Red,unit:cv,unit_name:"RCV".into(),weapon:"fixture".into(),damage:1.0,hp_left:999.0},harpoon_core::Visibility::Side(Side::Red));
    harpoon_core::exercise::update_alert(&mut w);
    assert!(!harpoon_core::exercise::locked(&w,Side::Red));
    w.tick=999;harpoon_core::sim::tick(&mut w,&[]);
    let result=w.result.unwrap();assert!(w.over && result.scored);assert_eq!(result.winner,Some(Side::Red));
    assert_eq!(result.blue_points,0.0);assert_eq!(result.red_points,100.0);
}

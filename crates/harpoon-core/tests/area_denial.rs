use harpoon_core::{load_scenario, observe, scenario_from_json, Database, Side, World};
use harpoon_core::geom::Vec2;
use harpoon_core::mission::adjudicate;

fn fixture() -> World {
    let sc = scenario_from_json(r#"{
      "id":"mission-test","name":"Mission test","duration_s":20,"seed":7,
      "weather":{"visibility_nm":12,"jam":[0,0]},
      "context":{"area_denial":{"defender":"Blue","center":[0,0],"radius_nm":10,"hold_s":3,"functional_hp_fraction":0.5,
          "protected_names":["B1","B2"],"intruder_names":["R1","R2"]}},
      "units":[
        {"side":"Blue","class":"cvn","name":"B1","pos":[-100,10]},
        {"side":"Blue","class":"cvn","name":"B2","pos":[-100,-10]},
        {"side":"Red","class":"cvn","name":"R1","pos":[30,10]},
        {"side":"Red","class":"cvn","name":"R2","pos":[30,-10]}] }"#).unwrap();
    load_scenario(sc, Database::built_in()).unwrap()
}

fn move_named(w: &mut World, name: &str, x: f64) {
    w.entities.values_mut().find(|e| e.name == name).unwrap().pos = Vec2::new(x,0.0);
}
fn disable(w: &mut World, name: &str) {
    let unit = w.entities.values_mut().find(|e| e.name == name).unwrap();
    unit.hp = unit.hp_max * 0.5;
}

#[test]
fn boundary_is_inside_but_one_second_short_is_not_a_win() {
    let mut w=fixture(); move_named(&mut w,"R1",10.0);
    for tick in 1..=2 { w.tick=tick; assert!(adjudicate(&mut w).is_none()); }
    w.tick=3; assert_eq!(adjudicate(&mut w),Some((Some(Side::Red),"mission_zone_held".into())));
}

#[test]
fn leaving_or_changing_carrier_does_not_preserve_a_hold() {
    let mut w=fixture(); move_named(&mut w,"R1",9.0);w.tick=1;adjudicate(&mut w);
    w.tick=2;adjudicate(&mut w);
    move_named(&mut w,"R1",10.001);move_named(&mut w,"R2",9.0);w.tick=3;
    assert!(adjudicate(&mut w).is_none());
    assert_eq!(observe(&w,Side::Red,0,0).mission.unwrap().own_hold_s,Some(1));
    disable(&mut w,"R2");w.tick=4;assert!(adjudicate(&mut w).is_none());
    assert!(w.mission_state.inside_since.is_empty());
}

#[test]
fn defender_observation_cannot_reveal_hidden_occupancy_or_damage() {
    let mut w=fixture();w.tick=4;
    let before=serde_json::to_value(observe(&w,Side::Blue,0,0)).unwrap();
    move_named(&mut w,"R1",1.0);disable(&mut w,"R2");adjudicate(&mut w);
    let after=serde_json::to_value(observe(&w,Side::Blue,0,0)).unwrap();
    assert_eq!(before,after);
    assert!(after["mission"]["own_hold_s"].is_null());
}

#[test]
fn hold_at_deadline_has_priority_and_timeout_is_task_based() {
    let mut w=fixture();move_named(&mut w,"R1",0.0);
    for tick in 18..=19 {w.tick=tick;assert!(adjudicate(&mut w).is_none());}
    w.tick=20;assert_eq!(adjudicate(&mut w).unwrap().0,Some(Side::Red));
    let mut w=fixture();w.tick=20;assert_eq!(adjudicate(&mut w),Some((Some(Side::Blue),"mission_area_denied".into())));
    let mut w=fixture();disable(&mut w,"B1");w.tick=20;
    assert_eq!(adjudicate(&mut w),Some((None,"mission_denied_with_carrier_loss".into())));
}

#[test]
fn simultaneous_disablement_is_a_draw_and_each_side_can_win_early() {
    let mut w=fixture();disable(&mut w,"R1");disable(&mut w,"R2");
    assert_eq!(adjudicate(&mut w).unwrap().0,Some(Side::Blue));
    disable(&mut w,"B1");disable(&mut w,"B2");
    assert_eq!(adjudicate(&mut w),Some((None,"mission_mutual_carrier_loss".into())));
    let mut w=fixture();disable(&mut w,"B1");disable(&mut w,"B2");
    assert_eq!(adjudicate(&mut w).unwrap().0,Some(Side::Red));
}

#[test]
fn serialized_hold_continues_and_simulation_ends_only_once() {
    let mut w=fixture();move_named(&mut w,"R1",0.0);
    harpoon_core::sim::tick(&mut w,&[]);
    let mut restored:World=serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    harpoon_core::sim::run_seconds(&mut w,5,&[]);
    harpoon_core::sim::run_seconds(&mut restored,5,&[]);
    assert!(w.over);assert_eq!(w.tick,3);assert_eq!(w.result.as_ref().unwrap().winner,Some(Side::Red));
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&restored).unwrap());
    let event_seq=w.event_seq;harpoon_core::sim::tick(&mut w,&[]);assert_eq!(w.event_seq,event_seq);
}

#[test]
fn invalid_mission_references_are_rejected() {
    let w=fixture();
    let source=serde_json::json!({"id":"bad","name":"bad","duration_s":20,"seed":1,"weather":w.weather,"context":w.context,"units":[]});
    let sc=scenario_from_json(&source.to_string()).unwrap();assert!(load_scenario(sc,Database::built_in()).is_err());
}

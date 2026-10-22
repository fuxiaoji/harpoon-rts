use harpoon_core::{
    load_scenario, observe, scenario_from_json, world::StationState, Database, EntityId, Side, Vec2,
};
#[test]
fn own_routes_and_station_are_observed_without_revealing_enemy_navigation_or_breaking_old_frames() {
    let sc=scenario_from_json(r#"{"id":"navigation-view","name":"view","seed":1,"duration_s":100,"weather":{"visibility_nm":1,"jam":[0,0]},"units":[{"side":"Blue","class":"cn_055","name":"LEADER","pos":[0,0]},{"side":"Blue","class":"cn_055","name":"FOLLOWER","pos":[0,5]},{"side":"Red","class":"cn_055","name":"SECRET","pos":[1000,1000]}]}"#).unwrap();
    let mut w = load_scenario(sc, Database::built_in()).unwrap();
    w.entity_mut(EntityId(1))
        .unwrap()
        .waypoints
        .push_back(Vec2::new(10.0, 20.0));
    w.entity_mut(EntityId(2)).unwrap().station = Some(StationState {
        leader: EntityId(1),
        bearing_rel_deg: 270.0,
        distance_nm: 5.0,
    });
    w.entity_mut(EntityId(3))
        .unwrap()
        .waypoints
        .push_back(Vec2::new(333.0, 444.0));
    let o = observe(&w, Side::Blue, 0, 0);
    assert_eq!(o.units.len(), 2);
    assert_eq!(o.units[0].navigation.as_ref().unwrap().waypoints[0].x, 10.0);
    assert_eq!(
        o.units[1]
            .navigation
            .as_ref()
            .unwrap()
            .station
            .as_ref()
            .unwrap()
            .leader,
        EntityId(1)
    );
    assert!(!serde_json::to_string(&o).unwrap().contains("SECRET"));
    let mut old = serde_json::to_value(&o.units[0]).unwrap();
    old.as_object_mut().unwrap().remove("navigation");
    let frame: harpoon_core::observe::UnitObs = serde_json::from_value(old.clone()).unwrap();
    assert!(frame.navigation.is_none());
    assert_eq!(
        serde_json::to_value(frame).unwrap(),
        old,
        "missing historical field stays omitted in checksum serialization"
    );
}

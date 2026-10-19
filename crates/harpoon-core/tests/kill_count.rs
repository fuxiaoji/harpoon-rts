use harpoon_core::damage::sink;
use harpoon_core::geom::Vec2;
use harpoon_core::observe::observe;
use harpoon_core::scenario::{load_scenario, scenario_from_json};
use harpoon_core::types::{ContactId, Side};
use harpoon_core::world::{Contact, World};
use harpoon_core::Database;

fn fixture(duration_s: i64) -> World {
    let src = format!(
        r#"{{
          "id":"kill-count-test","name":"Kill count test","duration_s":{duration_s},"seed":7,
          "victory":{{"target_vp":0,"kill_count_at_end":true}},
          "weather":{{"visibility_nm":12,"jam":[0,0]}},
          "units":[
            {{"side":"Blue","class":"cvn","name":"B1","pos":[-100,10]}},
            {{"side":"Blue","class":"cvn","name":"B2","pos":[-100,-10]}},
            {{"side":"Red","class":"cvn","name":"R1","pos":[30,10]}},
            {{"side":"Red","class":"cvn","name":"R2","pos":[30,-10]}}] }}"#
    );
    let sc = scenario_from_json(&src).unwrap();
    load_scenario(sc, Database::built_in()).unwrap()
}

fn id_of(w: &World, name: &str) -> harpoon_core::EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}

fn track(w: &mut World, side: Side, about: harpoon_core::EntityId) -> ContactId {
    let state = &mut w.sides[side.index()];
    let id = ContactId(state.next_contact + 1);
    state.next_contact += 1;
    state.contacts.insert(
        id,
        Contact {
            id,
            about,
            category: harpoon_core::types::PlatformKind::Ship,
            class_name: Some("CVN".into()),
            estimate: Vec2::new(30.0, 10.0),
            est_course_deg: 0.0,
            est_speed_kts: 0.0,
            bearing_only: false,
            quality: 1.0,
            last_update_tick: w.tick,
            first_tick: w.tick,
            stale: false,
            sensors: vec!["test".into()],
            offset: Vec2::new(0.0, 0.0),
            announced: true,
        },
    );
    id
}

#[test]
fn kills_are_counted_until_time_limit_and_survive_entity_cleanup() {
    let mut w = fixture(400);
    let r1 = id_of(&w, "R1");
    sink(&mut w, r1);
    assert_eq!(w.kill_counts(), (1, 0));
    harpoon_core::sim::run_seconds(&mut w, 400, &[]);
    assert!(w.over, "the clock must end the exercise");
    let result = w.result.as_ref().unwrap();
    assert_eq!(result.winner, Some(Side::Blue));
    assert_eq!(result.reason, "time limit: kill count");
    assert_eq!((result.blue_kills, result.red_kills), (1, 0));
    assert!(!w.entities.contains_key(&r1), "dead platform cleaned up");
    assert_eq!(w.kill_counts(), (1, 0), "counter must not depend on the entity");
}

#[test]
fn tie_on_kills_is_a_draw() {
    let mut w = fixture(10);
    let r1 = id_of(&w, "R1");
    let b1 = id_of(&w, "B1");
    sink(&mut w, r1);
    sink(&mut w, b1);
    harpoon_core::sim::run_seconds(&mut w, 10, &[]);
    let result = w.result.as_ref().unwrap();
    assert_eq!(result.winner, None);
    assert_eq!(result.reason, "time limit: kill count");
    assert_eq!((result.blue_kills, result.red_kills), (1, 1));
}

#[test]
fn observation_reports_only_own_confirmed_kills_and_losses() {
    let mut w = fixture(10);
    let r1 = id_of(&w, "R1");
    let b1 = id_of(&w, "B1");
    let cid = track(&mut w, Side::Blue, r1);
    sink(&mut w, r1);
    sink(&mut w, b1);

    let blue = observe(&w, Side::Blue, 0, 100);
    let victory = blue.victory.as_ref().expect("victory status is always projected");
    assert_eq!(victory.mode, "kill_count");
    assert_eq!(victory.own_confirmed_kills, 1, "contact resolved as sunk");
    assert_eq!(victory.own_losses, 1, "B1 was lost");
    let json = serde_json::to_value(&blue).unwrap();
    assert!(json["victory"].get("enemy_kills").is_none());
    assert!(json["victory"].get("enemy_losses").is_none());

    let red = observe(&w, Side::Red, 0, 100).victory.unwrap();
    assert_eq!(red.own_confirmed_kills, 0);
    assert_eq!(red.own_losses, 1);
    assert!(w.sides[Side::Blue.index()]
        .contacts
        .get(&cid)
        .is_some_and(|c| c.stale));
}

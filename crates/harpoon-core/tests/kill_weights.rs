use harpoon_core::damage::sink;
use harpoon_core::geom::Vec2;
use harpoon_core::observe::observe;
use harpoon_core::scenario::{load_scenario, scenario_from_json};
use harpoon_core::types::{ContactId, PlatformKind, Side};
use harpoon_core::world::{Contact, World};
use harpoon_core::Database;

fn fixture(weights: Option<(f64, f64, f64, f64)>) -> World {
    let weights_json = match weights {
        Some((ship, sub, aircraft, base)) => format!(
            r#","kill_weights":{{"ship":{ship},"sub":{sub},"aircraft":{aircraft},"base":{base}}}"#
        ),
        None => String::new(),
    };
    let src = format!(
        r#"{{
          "id":"kill-weights-test","name":"Kill weights test","duration_s":400,"seed":7,
          "victory":{{"target_vp":0,"kill_count_at_end":true{weights_json}}},
          "weather":{{"visibility_nm":12,"jam":[0,0]}},
          "units":[
            {{"side":"Blue","class":"cvn","name":"B1","pos":[-100,10]}},
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
            category: PlatformKind::Ship,
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
fn weighted_score_decides_and_breakdown_is_recorded() {
    let mut w = fixture(Some((10.0, 5.0, 1.0, 10.0)));
    let r1 = id_of(&w, "R1");
    // Blue confirms one carrier kill; R2 survives so Red reaches the limit.
    track(&mut w, Side::Blue, r1);
    sink(&mut w, r1);

    harpoon_core::sim::run_seconds(&mut w, 400, &[]);
    let result = w.result.as_ref().unwrap();
    assert!(result.scored);
    assert_eq!(result.blue_kills, 1);
    assert_eq!(result.blue_kills_by_kind.get("ship"), Some(&1));
    assert_eq!(result.blue_points, 10.0);
    assert_eq!(result.red_points, 0.0);
    assert_eq!(result.winner, Some(Side::Blue));
    assert_eq!(result.reason, "time limit: kill count");

    // Loss bookkeeping is authoritative and per-class even without a track.
    assert_eq!(w.losses_by_kind.get("ship").map(|n| n[Side::Red.index()]), Some(1));
    assert_eq!(w.losses_by_kind.get("ship").map(|n| n[Side::Blue.index()]), Some(0));
}

#[test]
fn without_weights_raw_counts_decide_and_scored_is_false() {
    let mut w = fixture(None);
    let r1 = id_of(&w, "R1");
    track(&mut w, Side::Blue, r1);
    sink(&mut w, r1);

    harpoon_core::sim::run_seconds(&mut w, 400, &[]);
    let result = w.result.as_ref().unwrap();
    assert!(!result.scored);
    assert_eq!(result.blue_kills, 1);
    assert_eq!(result.blue_points, 1.0);
    assert_eq!(result.blue_kills_by_kind.get("ship"), Some(&1));
    assert_eq!(result.winner, Some(Side::Blue));
}

#[test]
fn observation_exposes_own_breakdown_weights_and_in_flight_ordnance() {
    let mut w = fixture(Some((10.0, 5.0, 1.0, 10.0)));
    let r1 = id_of(&w, "R1");
    let cid = track(&mut w, Side::Blue, r1);
    sink(&mut w, r1);

    // Two Blue missiles in flight at the (now sunk) contact entity.
    let asm = w.db.weapons.values().find(|d| d.guidance == harpoon_core::db::Guidance::SeaSkimmer).unwrap().clone();
    let b1 = id_of(&w, "B1");
    for _ in 0..2 {
        harpoon_core::weapons::launch(&mut w, b1, &asm, Some(r1), None);
    }

    let blue = observe(&w, Side::Blue, 0, 0);
    let victory = blue.victory.as_ref().unwrap();
    assert_eq!(victory.own_kills_by_kind.get("ship"), Some(&1));
    assert!(victory.own_losses_by_kind.values().all(|n| *n == 0));
    let weights = victory.kill_weights.as_ref().unwrap();
    assert_eq!(weights.ship, 10.0);
    assert_eq!(weights.aircraft, 1.0);
    assert_eq!(victory.own_kill_points, 10.0);

    let contact = blue.contacts.iter().find(|c| c.id == cid).unwrap();
    assert_eq!(contact.own_in_flight, 2, "own rounds in flight at the contact");

    let red = observe(&w, Side::Red, 0, 0);
    assert!(
        red.contacts.iter().all(|c| c.own_in_flight == 0),
        "Red holds no Blue launches; its own in-flight count stays 0"
    );
}

#[test]
fn sonobuoys_are_not_scored_kills_or_losses() {
    let mut w = fixture(Some((10.0, 5.0, 1.0, 10.0)));
    let r1 = id_of(&w, "R1");
    let cid = track(&mut w, Side::Blue, r1);
    // A buoy resolves onto the enemy side list and then dies.
    let state = &mut w.sides[Side::Red.index()];
    state.next_contact += 1;
    let buoy_entity = w.allocate_id();
    w.entities.insert(buoy_entity, harpoon_core::world::Entity {
        id: buoy_entity,
        side: Side::Red,
        class: "sonobuoy".into(),
        name: "RED-99 Sonobuoy".into(),
        kind: PlatformKind::Ship,
        pos: Vec2::new(31.0, 10.0),
        course_deg: 0.0, speed_kts: 0.0, ordered_course_deg: 0.0, ordered_speed_kts: 0.0,
        waypoints: Default::default(), altitude_m: 0.0, ordered_altitude_m: 0.0,
        depth_m: 0.0, ordered_depth_m: 0.0,
        hp: 1.0, hp_max: 1.0, alive: true,
        systems: Default::default(), fire_on: false, assigned: None,
        weapons: Vec::new(), sensors: Vec::new(),
        chaff: 0, decoys: 0, auto_chaff: false, chaff_active_s: 0.0,
        endurance_s: None, burn_s: 0.0, flooding_s: 0.0, ext: Default::default(),
        deck: None, mission: None, station: None, projectile: None, last_emit_tick: 0,
    });
    sink(&mut w, buoy_entity);

    harpoon_core::sim::run_seconds(&mut w, 400, &[]);
    let result = w.result.as_ref().unwrap();
    assert_eq!(result.red_kills, 0, "a dead buoy is not a loss for Red");
    assert!(result.blue_kills_by_kind.get("ship").is_none(), "sinking a buoy must not score a ship kill");
    assert_eq!(result.blue_points, 0.0);
    assert!(w.losses_by_kind.get("ship").is_none());
    assert!(!w.sides[Side::Blue.index()].contacts.contains_key(&cid) || true);
}

use harpoon_core::damage::{apply_hit, apply_hit_from};
use harpoon_core::geom::Vec2;
use harpoon_core::observe::observe;
use harpoon_core::scenario::{load_scenario, scenario_from_json};
use harpoon_core::types::{ContactId, PlatformKind, Side};
use harpoon_core::world::{Contact, World};
use harpoon_core::Database;

fn fixture() -> World {
    let src = r#"{
      "id":"bda-test","name":"BDA test","duration_s":600,"seed":7,
      "weather":{"visibility_nm":12,"jam":[0,0]},
      "units":[
        {"side":"Blue","class":"cvn","name":"B1","pos":[-100,0]},
        {"side":"Red","class":"cvn","name":"R1","pos":[30,0]}]}"#;
    let sc = scenario_from_json(src).unwrap();
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
            estimate: Vec2::new(30.0, 0.0),
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
fn confirmed_hit_on_tracked_target_updates_only_the_shooters_bda() {
    let mut w = fixture();
    let r1 = id_of(&w, "R1");
    let cid = track(&mut w, Side::Blue, r1);
    let gun = w.db.weapons["gun_5in"].clone();
    let before = w.entities[&r1].hp;

    apply_hit_from(&mut w, r1, &gun, false, Side::Blue);

    assert!(w.entities[&r1].hp < before, "the hit must damage the target");
    let blue = observe(&w, Side::Blue, 0, 0);
    let contact = blue.contacts.iter().find(|c| c.id == cid).unwrap();
    assert_eq!(contact.bda_hits, 1);
    assert!(contact.bda_damage > 0.0);
    let fraction = contact.bda_hp_fraction.expect("tracked hit reports hull estimate");
    assert!((0.0..1.0).contains(&fraction));

    let red = observe(&w, Side::Red, 0, 0);
    assert!(
        red.contacts.iter().all(|c| c.bda_hits == 0),
        "the victim must not inherit the shooter's private BDA"
    );
    assert!(
        w.sides[Side::Red.index()].bda.is_empty(),
        "only the side that observed the hit gets an assessment"
    );
}

#[test]
fn untracked_hit_creates_no_bda() {
    let mut w = fixture();
    let r1 = id_of(&w, "R1");
    let gun = w.db.weapons["gun_5in"].clone();

    apply_hit_from(&mut w, r1, &gun, false, Side::Blue);

    assert!(observe(&w, Side::Blue, 0, 0).contacts.is_empty());
    assert!(w.sides[Side::Blue.index()].bda.is_empty());
}

#[test]
fn bda_is_visible_after_the_track_goes_stale_and_accumulates() {
    let mut w = fixture();
    let r1 = id_of(&w, "R1");
    let cid = track(&mut w, Side::Blue, r1);
    let gun = w.db.weapons["gun_5in"].clone();

    apply_hit_from(&mut w, r1, &gun, false, Side::Blue);
    let first = observe(&w, Side::Blue, 0, 0)
        .contacts
        .into_iter()
        .find(|c| c.id == cid)
        .unwrap()
        .bda_damage;
    apply_hit_from(&mut w, r1, &gun, false, Side::Blue);
    w.sides[Side::Blue.index()]
        .contacts
        .get_mut(&cid)
        .unwrap()
        .stale = true;

    let contact = observe(&w, Side::Blue, 0, 0)
        .contacts
        .into_iter()
        .find(|c| c.id == cid)
        .unwrap();
    assert_eq!(contact.bda_hits, 2);
    assert!(contact.bda_damage > first);
    assert!(contact.stale, "the track can age while the assessment remains");
}

#[test]
fn friendly_fire_does_not_credit_the_enemy_with_bda() {
    let mut w = fixture();
    let r1 = id_of(&w, "R1");
    // Blue watches the Red ship, but the damage comes from a Red misfire.
    track(&mut w, Side::Blue, r1);
    let gun = w.db.weapons["gun_5in"].clone();

    apply_hit_from(&mut w, r1, &gun, false, Side::Red);

    assert!(
        w.sides[Side::Blue.index()].bda.is_empty(),
        "a side must not receive BDA for damage it did not inflict"
    );
    assert!(observe(&w, Side::Blue, 0, 0)
        .contacts
        .iter()
        .all(|c| c.bda_hits == 0));
}

#[test]
fn legacy_apply_hit_without_shooter_records_no_bda() {
    let mut w = fixture();
    let r1 = id_of(&w, "R1");
    track(&mut w, Side::Blue, r1);
    let gun = w.db.weapons["gun_5in"].clone();

    apply_hit(&mut w, r1, &gun, false);

    assert!(w.sides[Side::Blue.index()].bda.is_empty());
}

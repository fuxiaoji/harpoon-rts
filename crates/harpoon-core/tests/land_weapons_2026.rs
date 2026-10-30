//! Production missile movement consumes §11.2, §11.1 and §14.1.6.
//! Guidance choice and physical layout below are explicit test assumptions;
//! target DP/armor and Tomahawk DP/penetration are source bindings.
use harpoon_core::{rules::surface2026, Database, EntityId, Scenario, Side, Vec2, World};
use serde_json::json;

fn fixture(template: &str, guidance: bool) -> (World, EntityId, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json"))
        .unwrap()
        .merged(
            &Database::from_json(include_str!(
                "../../../database/volumes/scenario_exercise_assets.json"
            ))
            .unwrap(),
        );
    let mut config = json!({"source":"scenario_assumption: source-table test layout",
        "facilities":{"Facility":{"template":template,"custom":null,"source":"English2026 PDF118 generic target"}},
        "missiles":{}});
    if guidance {
        config["missiles"] = json!({"us_tomahawk":{"family":"Lacm","guidance":"T2","satellite":true,"nuclear":false,
            "source":"scenario_assumption: T2 test guidance, not a Tomahawk accuracy claim"}});
    }
    let scenario:Scenario = serde_json::from_value(json!({"id":"land-weapon-source","name":"Land weapon source",
        "duration_s":1000,"seed":1,"context":{"rules_profile":"hv2026-rts-v4","ground_2026":config},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_burke_2a","pos":[0,0],"fire_on":false},
            {"side":"Red","class":"exercise_airbase","name":"Facility","pos":[0,1],"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let shooter = w
        .entities
        .values()
        .find(|e| e.side == Side::Blue)
        .unwrap()
        .id;
    let target = w
        .entities
        .values()
        .find(|e| e.name == "Facility")
        .unwrap()
        .id;
    // def.pk must never replace the source guidance-size table.
    w.db.weapons.get_mut("us_tomahawk").unwrap().pk = 0.;
    (w, shooter, target)
}
fn terminal(mut w: World, shooter: EntityId, target: EntityId, seed: u64) -> (World, EntityId) {
    w.rng = harpoon_core::Rng::new(seed);
    let def = w.db.weapons["us_tomahawk"].clone();
    let id = harpoon_core::weapons::launch(&mut w, shooter, &def, Some(target), None).unwrap();
    w.entity_mut(id).unwrap().pos = Vec2::new(0., 0.99);
    harpoon_core::weapons::advance_projectiles(&mut w);
    (w, id)
}
#[test]
fn actual_lacm_uses_guidance_and_size_table_and_destroys_radar_without_sinking_it() {
    let (base, s, t) = fixture("search_mfc_radar", true);
    let mut saw_hit = false;
    let mut saw_miss = false;
    for seed in 1..=100 {
        let (w, id) = terminal(base.clone(), s, t, seed);
        let o = &w.rules.aaw.as_ref().unwrap().surface.land_outcomes[&id];
        assert_eq!(o.probability, 0.20); // T2 vs G, PDF118.
        assert!(!w.entity(id).unwrap().alive);
        assert!(!w.rules.pending_adjudications.contains_key(&id));
        if o.hit {
            saw_hit = true;
            assert_eq!(o.damage, Some(51.)); // US Navy2025 PDF77 TacticalTomahawkIV.
            assert_eq!(w.entity(t).unwrap().hp, 0.);
            assert!(
                w.entity(t).unwrap().alive,
                "land rubble remains an entity, not a sinking ship"
            );
            assert!(w.entity(t).unwrap().sensors.is_empty());
            assert!(w.rules.ground.as_ref().unwrap().facilities[&t].destroyed);
        } else {
            saw_miss = true;
            assert_eq!(w.entity(t).unwrap().hp, 20.);
        }
    }
    assert!(saw_hit && saw_miss);
}
#[test]
fn missing_guidance_preserves_complete_projectile_and_never_falls_back_to_pk() {
    let (base, s, t) = fixture("search_mfc_radar", false);
    let (mut w, id) = terminal(base, s, t, 1);
    assert!(w.rules.pending_adjudications.contains_key(&id));
    assert_eq!(w.entity(t).unwrap().hp, 20.);
    let p = serde_json::to_value(w.entity(id).unwrap()).unwrap();
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..10 {
        harpoon_core::weapons::advance_projectiles(&mut restored);
    }
    assert_eq!(
        p,
        serde_json::to_value(restored.entity(id).unwrap()).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&w.rules.pending_adjudications).unwrap(),
        serde_json::to_value(&restored.rules.pending_adjudications).unwrap()
    );
    surface2026::flush_land_hits(&mut w);
    assert_eq!(w.entity(t).unwrap().hp, 20.);
}
#[test]
fn surviving_land_target_retains_original_hit_die_when_missile_extra_scope_is_unresolved() {
    let (base, s, t) = fixture("runway", true);
    let (mut w, id) = (1..=100)
        .map(|seed| terminal(base.clone(), s, t, seed))
        .find(|(w, id)| w.rules.aaw.as_ref().unwrap().surface.land_outcomes[id].hit)
        .unwrap();
    let outcome =
        serde_json::to_value(&w.rules.aaw.as_ref().unwrap().surface.land_outcomes[&id]).unwrap();
    assert_eq!(w.entity(t).unwrap().hp, 100.);
    assert_eq!(w.rules.ground.as_ref().unwrap().facilities[&t].cuts, 0);
    assert!(w.rules.pending_adjudications[&id]
        .rule
        .contains("additional-critical"));
    let before = serde_json::to_value(w.entity(id).unwrap()).unwrap();
    for _ in 0..10 {
        harpoon_core::weapons::advance_projectiles(&mut w);
    }
    assert_eq!(before, serde_json::to_value(w.entity(id).unwrap()).unwrap());
    assert_eq!(
        outcome,
        serde_json::to_value(&w.rules.aaw.as_ref().unwrap().surface.land_outcomes[&id]).unwrap()
    );
}

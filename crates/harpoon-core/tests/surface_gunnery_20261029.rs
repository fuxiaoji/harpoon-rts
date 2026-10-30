//! Public native lookout -> own Fire -> common damage; fictional service fit.
use harpoon_core::rules::detection2026 as detection;
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};
fn world(obstruction: bool) -> World {
    world_for(obstruction, false)
}
fn world_for(obstruction: bool, carrier: bool) -> World {
    world_at(obstruction,carrier,11.)
}
fn world_at(obstruction: bool, carrier: bool,range:f64) -> World {
    let seed = (0..100000)
        .find(|s| {
            detection::trial(
                &mut detection::State::default(),
                *s,
                1,
                "5.8.visual:1:visual",
                180,
            )
            .0 <= 5
                && {
                    let mut r = harpoon_core::Rng::new(*s).derive(harpoon_core::Rng::hash_tag(
                        61,
                        1,
                        "8.3:61:1:cn_pj45a:2",
                    ));
                    1 + (r.next_f64() * 100.) as u8 <= if carrier { 5 } else { 80 }
                }
                && (!carrier || {
                    let mut r = Rng::new(*s).derive(Rng::hash_tag(0, 2, "14.1.2:gun_deck:2:0"));
                    let die=1+(r.next_f64()*100.)as u8;
                    if range>12.7 {(61..=70).contains(&die)}else{die<=60}
                })
        })
        .unwrap();
    let mut input = json!({"id":"surface-gunnery-source","name":"scenario_assumption: native surface gun service","seed":seed,"duration_s":5000,
 "weather":{"visibility_nm":20,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4",
 "detection_2026":{"bindings":{"cn_052d:visual":{"type":"visual","source":"scenario_assumption: native lookout"}},"targets":{"cn_052d":{"source":"scenario_assumption: C visual signature","size":2,"noise":null,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}},"environment":{"source":"scenario_assumption: explicit clear daylight","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}},
 "surface_gunnery_2026":{"source":"scenario_assumption: qualified test service, no claim of actual director fitting","units":{"SHOOTER":{"source":"scenario_assumption: explicit two-barrel test mount and clear conditions","weapon":"cn_pj45a","standard":"gs5","barrels":2,"arcs":[[0,360]],"under_fire":true,"local_control":false,"radar_gfc":false,"reaction_only":false,"terrain_clear":true,"other_conditions_clear":true,"ammunition_per_attack":2}}}},
 "units":[{"side":"Blue","class":"cn_052d","name":"SHOOTER","pos":[0,0],"speed_kts":0,"fire_on":false},{"side":"Red","class":"cn_052d","name":"TARGET","pos":[0,3],"course_deg":90,"speed_kts":0,"fire_on":false}]});
    if obstruction {
        input["units"].as_array_mut().unwrap().push(json!({"side":"Blue","class":"cn_052d","name":"OBSTRUCTION","pos":[0,1],"speed_kts":0,"fire_on":false}));
    }
    if carrier {
        input["units"][1]["class"] = json!("cn_001");
        input["units"][1]["pos"] = json!([0, range]);
        let mut p = input["context"]["detection_2026"]["targets"]["cn_052d"].clone();
        p["size"] = json!(0);
        input["context"]["detection_2026"]["targets"]["cn_001"] = p;
    }
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    // The experimental service does not certify Type052D hull fields. Its
    // C-sized unarmored test body is declared before World construction;
    // the cn_001 carrier retains its separately sourced real hull row.
    db.platforms.get_mut("cn_052d").unwrap().rule_data=Some(harpoon_core::rules::damage_runtime::PlatformDamage{
        source:"scenario_assumption: C unarmored laboratory hull, not a Type052D Annex certification".into(),
        size:"C".into(),role:"surface".into(),belt_cm:0.,deck_cm:0.,..Default::default()
    });
    db.platforms.get_mut("cn_052d").unwrap().sensors = vec!["visual".into()];
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(), db).unwrap()
}
fn advance(w: &mut World, n: usize) {
    for _ in 0..n {
        harpoon_core::sim::tick(w, &[]);
    }
}
fn fire(w: &World) -> Command {
    let c = w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == EntityId(2) && !c.stale)
        .expect("real native visual contact required")
        .id;
    Command::Fire {
        unit: EntityId(1),
        contact: c,
        weapon: "cn_pj45a".into(),
        rounds: 1,
    }
}
fn rows(w: &World) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "8.3.source_resolution" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn gun_deck_rows(w: &World) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "14.1.2.automatic_gun_deck" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
#[test]
fn native_long_gun_hit_adds_one_actual_deck_critical_per_turn_with_restore() {
    let mut w = world_for(false, true);
    advance(&mut w, 60);
    let c = fire(&w);
    c.validate_source(&w, Side::Blue).unwrap();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, c.clone()), (Side::Blue, c)]);
    let source = rows(&w);
    assert_eq!(source.len(), 1);
    assert_eq!(source[0]["range_band"], 2);
    assert_eq!(source[0]["hit"], true);
    assert!(
        gun_deck_rows(&w).is_empty(),
        "aggregate the whole original Tactical Turn before the one additional occurrence roll"
    );
    assert!(
        w.rules
            .aaw
            .as_ref()
            .unwrap()
            .surface
            .gunnery
            .impacts
            .is_empty(),
        "shared damage must consume actual source proof"
    );
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    advance(&mut w, 120);
    advance(&mut saved, 120);
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    let deck = gun_deck_rows(&w);
    assert_eq!(deck.len(), 1);
    assert_eq!(deck[0]["chance_percent"], 60);
    assert_eq!(deck[0]["applied"], true);
    assert_eq!(
        w.entity(EntityId(2)).unwrap().ext.get("flight_deck_hit"),
        Some(&1.)
    );
    advance(&mut w, 180);
    assert_eq!(
        gun_deck_rows(&w).len(),
        1,
        "no reroll or further automatic deck without another actual qualifying turn"
    );
}
#[test]
fn native_extreme_hit_uses_seventy_percent_with_the_original_sixty_one_to_seventy_roll() {
    let mut w=world_at(false,true,15.);advance(&mut w,60);let c=fire(&w);
    c.validate_source(&w,Side::Blue).unwrap();harpoon_core::sim::tick(&mut w,&[(Side::Blue,c)]);
    assert_eq!(rows(&w)[0]["range_band"],3);assert_eq!(rows(&w)[0]["hit"],true);
    advance(&mut w,120);let d=gun_deck_rows(&w);assert_eq!(d.len(),1);
    assert!((61..=70).contains(&d[0]["d100"].as_u64().unwrap()));assert_eq!(d[0]["chance_percent"],70);
    assert_eq!(d[0]["applied"],true);assert_eq!(w.entity(EntityId(2)).unwrap().ext.get("flight_deck_hit"),Some(&1.));
}
#[test]
fn native_surface_fire_uses_source_gs5_cell_and_one_attack_slot() {
    let mut w = world(false);
    advance(&mut w, 60);
    let c = fire(&w);
    let before = serde_json::to_vec(&w).unwrap();
    c.validate_source(&w, Side::Blue).unwrap();
    assert_eq!(before, serde_json::to_vec(&w).unwrap());
    let hp_before = w.entity(EntityId(2)).unwrap().hp;
    let ammo = w
        .entity(EntityId(1))
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "cn_pj45a")
        .unwrap()
        .ammo;
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, c.clone()), (Side::Blue, c.clone())]);
    let r = rows(&w);
    assert_eq!(r[0]["hit"], true);
    assert!(
        w.entity(EntityId(2)).map_or(true, |e| e.hp < hp_before),
        "a genuine source hit must enter shared damage"
    );
    assert_eq!(
        r.len(),
        1,
        "one gun attack, duplicate intent must not buy more dice"
    );
    assert!((r[0]["probability"].as_f64().unwrap() - 0.8).abs() < 1e-9);
    assert_eq!(
        ammo - w
            .entity(EntityId(1))
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "cn_pj45a")
            .unwrap()
            .ammo,
        2
    );
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    advance(&mut w, 5);
    advance(&mut saved, 5);
    assert_eq!(
        serde_json::to_value(w).unwrap(),
        serde_json::to_value(saved).unwrap()
    );
}
#[test]
fn friendly_hull_in_the_source_fire_cone_blocks_without_expenditure() {
    let mut w = world(true);
    advance(&mut w, 60);
    let c = fire(&w);
    let ammo = w
        .entity(EntityId(1))
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "cn_pj45a")
        .unwrap()
        .ammo;
    let before = serde_json::to_vec(&w).unwrap();
    let err = c.validate_source(&w, Side::Blue).unwrap_err();
    assert!(err.contains("friendly"));
    assert_eq!(before, serde_json::to_vec(&w).unwrap());
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, c)]);
    assert!(rows(&w).is_empty());
    assert_eq!(
        ammo,
        w.entity(EntityId(1))
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "cn_pj45a")
            .unwrap()
            .ammo
    );
}

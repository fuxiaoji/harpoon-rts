//! Public native lookout -> own Fire -> common damage; fictional service fit.
use harpoon_core::rules::detection2026 as detection;
use harpoon_core::{Command, Database, EntityId, EventKind, Rng, Scenario, Side, World};
use serde_json::{json, Value};


fn world_at(weapon: &str) -> World {
    let obstruction = false; let carrier = false; let range = 11.;
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
                        &format!("8.3:61:1:{weapon}:2"),
                    ));
                    1 + (r.next_f64() * 100.) as u8 <= if carrier { 5 } else { 50 }
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
 "surface_gunnery_2026":{"source":"scenario_assumption: qualified test service, no claim of actual director fitting","units":{"SHOOTER":{"source":"scenario_assumption: explicit two-barrel test mount and clear conditions","weapon":weapon,"standard":"gs5","barrels":2,"arcs":[[0,360]],"under_fire":true,"local_control":false,"radar_gfc":false,"reaction_only":false,"terrain_clear":true,"other_conditions_clear":true,"ammunition_per_attack":2}}}},
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
    db.platforms.get_mut("cn_052d").unwrap().weapons = vec![harpoon_core::db::WeaponFit { weapon:weapon.into(), mounts:1, ammo:50 }];
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(), db).unwrap()
}
fn advance(w: &mut World, n: usize) {
    for _ in 0..n {
        harpoon_core::sim::tick(w, &[]);
    }
}
fn fire(w: &World, weapon: &str) -> Command {
    let c = w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == EntityId(2) && !c.stale)
        .expect("real native visual contact required")
        .id;
    Command::Fire {
        unit: EntityId(1),
        contact: c,
        weapon: weapon.into(),
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

#[test]
fn two_verified_annex_guns_fire_from_native_visual_and_damage() {
    for (weapon, band, damage) in [("cw_5in_mk45",0,24.), ("us_mk110",1,16.)] {
        let mut w=world_at(weapon); advance(&mut w,60);
        let hp=w.entity(EntityId(2)).unwrap().hp;
        let c=fire(&w,weapon); c.validate_source(&w,Side::Blue).unwrap();
        harpoon_core::sim::tick(&mut w,&[(Side::Blue,c)]);
        let r=rows(&w); assert_eq!(r.len(),1); assert_eq!(r[0]["hit"],true);
        assert_eq!(r[0]["range_band"],band); assert_eq!(r[0]["damage_before_armor"],damage);
        assert!(w.entity(EntityId(2)).map_or(true,|e|e.hp<hp));
    }
}

#[test]
fn chinese_annex_he_guns_fire_from_native_visual_and_damage() {
    for (weapon, band, damage) in [("cn_pj26",1,13.), ("cn_pj87",1,17.), ("cn_pj33b",0,19.)] {
        let mut w=world_at(weapon); advance(&mut w,60);
        let hp=w.entity(EntityId(2)).unwrap().hp;
        let c=fire(&w,weapon); c.validate_source(&w,Side::Blue).unwrap();
        harpoon_core::sim::tick(&mut w,&[(Side::Blue,c)]);
        let r=rows(&w); assert_eq!(r.len(),1); assert_eq!(r[0]["hit"],true);
        assert_eq!(r[0]["range_band"],band); assert_eq!(r[0]["damage_before_armor"],damage);
        assert!(w.entity(EntityId(2)).map_or(true,|e|e.hp<hp));
    }
}

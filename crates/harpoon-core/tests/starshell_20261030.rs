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
fn world_at(obstruction: bool, carrier: bool,range:f64) -> World { world_at_day(obstruction,carrier,range,false) }
fn world_at_day(obstruction: bool, carrier: bool,range:f64,night:bool) -> World {world_at_mount(obstruction,carrier,range,night,false)}
fn world_at_mount(obstruction: bool, carrier: bool,range:f64,night:bool,multiple:bool) -> World {
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
                        if night {182} else {61},
                        1,
                        if night {"8.3:182:1:cn_pj45a:2"} else {"8.3:61:1:cn_pj45a:2"},
                    ));
                    1 + (r.next_f64() * 100.) as u8 <= if carrier || night { 5 } else { 80 }
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
 "surface_gunnery_2026":{"source":"scenario_assumption: qualified test service, no claim of actual director fitting","units":{"SHOOTER":{"source":"scenario_assumption: explicit two-barrel test mount and clear conditions","weapon":"cn_pj45a","standard":"gs5","barrels":2,"arcs":[[0,360]],"under_fire":true,"local_control":false,"radar_gfc":false,"reaction_only":false,"terrain_clear":true,"other_conditions_clear":true,"ammunition_per_attack":2,"starshell_stock":4,"starshell_per_turn":2,"starshell_mount_role":"primary"}}}},
 "units":[{"side":"Blue","class":"cn_052d","name":"SHOOTER","pos":[0,0],"speed_kts":0,"fire_on":false},{"side":"Red","class":"cn_052d","name":"TARGET","pos":[0,3],"course_deg":90,"speed_kts":0,"fire_on":false}]});
    if night {
        // cn_052d has no source physical_sizes row, so radar_height rejects it.
        // Use source-recognized Burke radar geometry; the PJ45A mount is an
        // explicitly authored laboratory fit, not a real Burke installation.
        input["day_night"]=json!("Night");
        input["context"]["radar_environment"]=json!({"sea_state":0,"precipitation":"None"});
        input["context"]["detection_2026"]["environment"]["visibility_percent"]=json!(10);
        input["context"]["detection_2026"]["targets"]["us_burke_2a"]=input["context"]["detection_2026"]["targets"]["cn_052d"].clone();
        input["context"]["detection_2026"]["targets"]["us_burke_2a"]["size"]=json!(1);
        input["context"]["surface_gunnery_2026"]["units"]["SHOOTER"]["starshell_mount_role"]=json!("secondary");
        input["units"][0]["class"]=json!("us_burke_2a"); input["units"][1]["class"]=json!("us_burke_2a");
    }
    if multiple {
        input["context"]["surface_gunnery_2026"]["units"]["SHOOTER"]["starshell_physical_index"]=json!(1);
        input["context"]["expanded_damage_2026"]=json!({"source":"scenario_assumption: individual lab mounts", "units":{"SHOOTER":{"source":"scenario_assumption: distinct mount identities","components":{
            "mount0":{"source":"scenario_assumption: physical 0","kind":"weapon_mount","binding":"cn_pj45a","mount_index":0,"physical":0,"ammunition":2,"location":"topside"},
            "mount1":{"source":"scenario_assumption: physical 1","kind":"weapon_mount","binding":"cn_pj45a","mount_index":0,"physical":1,"ammunition":2,"location":"topside"}}}}});
    }
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
    if multiple {
        let mut gun=db.platforms["cn_052d"].weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().clone();
        gun.mounts=2; db.platforms.get_mut("cn_052d").unwrap().weapons=vec![gun];
    }
    if !night { db.platforms.get_mut("cn_052d").unwrap().sensors = vec!["visual".into()]; }
    else {
        let gun=db.platforms["cn_052d"].weapons.iter().find(|m|m.weapon=="cn_pj45a").unwrap().clone();
        let body=db.platforms.get_mut("us_burke_2a").unwrap();
        body.weapons.push(gun);
        body.sensors=vec!["cn_ljg346a".into()]; // authored laboratory mount; source radar card exists
        body.rule_data=Some(harpoon_core::rules::damage_runtime::PlatformDamage{source:"scenario_assumption: B test hull".into(),size:"B".into(),role:"surface".into(),belt_cm:0.,deck_cm:0.,..Default::default()});
    }
    harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(), db).unwrap()
}
fn advance(w: &mut World, n: usize) {
    for _ in 0..n {
        harpoon_core::sim::tick(w, &[]);
    }
}

#[test]
fn starshell_uses_physical_slot_and_stock_without_damage_and_restores() {
    let mut w=world(false); advance(&mut w,60);
    let contact=w.sides[0].contacts.values().find(|c|c.about==EntityId(2)&&!c.stale).unwrap().id;
    let action=detection::DetectionAction::FireStarshell{contact,weapon:"cn_pj45a".into()};
    let command=Command::Detection{unit:EntityId(1),action};
    command.validate_source(&w,Side::Blue).unwrap();
    let before=w.entity(EntityId(2)).unwrap().hp;
    let mut restored:World=serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,command.clone())]);
    harpoon_core::sim::tick(&mut restored,&[(Side::Blue,command)]);
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&restored).unwrap());
    assert_eq!(w.entity(EntityId(2)).unwrap().hp,before);
    let g=&w.rules.aaw.as_ref().unwrap().surface.gunnery;
    assert_eq!(g.fits[&EntityId(1)].starshell_stock,Some(2));
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="5.8.8.starshell_resolution")));
}

#[test]
fn actual_starshell_does_not_grant_illumination_to_unqualified_observer() {
    let mut w=world_at_day(false,false,11.,true);
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::SetRadar{unit:EntityId(1),active:true})]);
    advance(&mut w,180);
    let contact=w.sides[0].contacts.values().find(|c|c.about==EntityId(2)&&!c.stale).unwrap().id;
    let own=harpoon_core::rules::communications2026::own_view(&w,EntityId(1)).unwrap();
    assert!(own["reports"].as_array().unwrap().iter().any(|r|r["contact"]==json!(contact) && r["sensor"].as_str().is_some_and(|s|s=="cn_ljg346a")),"real own radar report required");
    let cmd=Command::Detection{unit:EntityId(1),action:detection::DetectionAction::FireStarshell{contact,weapon:"cn_pj45a".into()}};
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]);
    let source=&w.rules.aaw.as_ref().unwrap().surface.gunnery;
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="5.8.8.starshell_resolution")));
    assert!(source.illumination.values().any(|light|light.actor==EntityId(1) && light.effect.illuminated && light.physical_index==0),"actual indexed physical mount creates light");
    assert!(harpoon_core::rules::surface2026::gunnery::illumination(&w,EntityId(1),EntityId(2)).is_some(),"firing ship qualifies in normal night");
    assert_eq!(harpoon_core::rules::surface2026::gunnery::illumination(&w,EntityId(2),EntityId(2)),None);
    let seen=detection::own_view(&w,Side::Red,EntityId(2)).unwrap();
    assert_eq!(seen["visible_lights"],json!([{"kind":"starshell_light"}]));
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,unit:Some(unit),..} if rule=="5.8.8.light_seen" && *unit==EntityId(2))));
    let restored:World=serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    assert_eq!(detection::own_view(&restored,Side::Red,EntityId(2)).unwrap()["visible_lights"],seen["visible_lights"]);
    assert!(!seen["visible_lights"].to_string().contains("target"));
}

#[test]
fn actual_star_light_expires_at_tactical_turn_boundary() {
    let mut w=world_at_day(false,false,11.,true);
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::SetRadar{unit:EntityId(1),active:true})]);
    advance(&mut w,180);
    let contact=w.sides[0].contacts.values().find(|c|c.about==EntityId(2)&&!c.stale).unwrap().id;
    let cmd=Command::Detection{unit:EntityId(1),action:detection::DetectionAction::FireStarshell{contact,weapon:"cn_pj45a".into()}};
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]);
    assert!(!harpoon_core::rules::surface2026::gunnery::visible_lights(&w,EntityId(1)).is_empty());
    let mut restored:World=serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    while w.tick<360 {harpoon_core::sim::tick(&mut w,&[]);harpoon_core::sim::tick(&mut restored,&[]);}
    assert!(harpoon_core::rules::surface2026::gunnery::visible_lights(&w,EntityId(1)).is_empty());
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(&restored).unwrap());
}

#[test]
fn two_physical_mounts_can_fire_star_and_ordinary_in_same_turn() {
    let mut w=world_at_mount(false,false,11.,false,true); advance(&mut w,60);
    let contact=w.sides[0].contacts.values().find(|c|c.about==EntityId(2)&&!c.stale).unwrap().id;
    let star=Command::Detection{unit:EntityId(1),action:detection::DetectionAction::FireStarshell{contact,weapon:"cn_pj45a".into()}};
    let ordinary=Command::Fire{unit:EntityId(1),contact,weapon:"cn_pj45a".into(),rounds:1};
    let mut restored:World=serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,star.clone()),(Side::Blue,ordinary.clone())]);
    harpoon_core::sim::tick(&mut restored,&[(Side::Blue,ordinary),(Side::Blue,star)]);
    let rules:Vec<_>=w.events.iter().filter_map(|e|match &e.kind {EventKind::RulesEvent{rule,..}=>Some(rule.as_str()),_=>None}).collect();
    assert!(rules.contains(&"5.8.8.starshell_resolution"));
    assert!(rules.contains(&"8.3.source_resolution"));
    assert_eq!(w.rules.aaw.as_ref().unwrap().surface.gunnery.fits[&EntityId(1)].starshell_stock,Some(2));
    // Commands are reversed; compare actual inventories and independent slot outcomes.
    assert_eq!(serde_json::to_value(&w.entity(EntityId(1)).unwrap().weapons).unwrap(),serde_json::to_value(&restored.entity(EntityId(1)).unwrap().weapons).unwrap());
    assert_eq!(w.rules.aaw.as_ref().unwrap().surface.gunnery.last_fire,restored.rules.aaw.as_ref().unwrap().surface.gunnery.last_fire);
}

#[test]
fn adjudicated_weapon_critical_ends_only_damaged_starshell_mount() {
    let mut w=world_at_mount(false,false,11.,false,true); advance(&mut w,60);
    let contact=w.sides[0].contacts.values().find(|c|c.about==EntityId(2)&&!c.stale).unwrap().id;
    let cmd=Command::Detection{unit:EntityId(1),action:detection::DetectionAction::FireStarshell{contact,weapon:"cn_pj45a".into()}};
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]);
    assert!(!w.rules.aaw.as_ref().unwrap().surface.gunnery.illumination.is_empty());
    // Preselect only the original two-way physical selection die, no state injection.
    let identity=(0..1000).map(|i|format!("star-critical-{i}")).find(|name|{
        let mut r=w.rng.derive(Rng::hash_tag(w.tick,1,name)); r.next_f64()>=0.5 && (1+(r.next_f64()*10.) as u8)!=10
    }).unwrap();
    harpoon_core::rules::expanded_damage2026::source_critical(&mut w,EntityId(1),&identity,"weapon",
        harpoon_core::rules::expanded_damage2026::CriticalContext{source:"scenario_assumption: preadjudicated weapon critical".into(),dp:0.,penetrated:true,underwater:false,airburst:false,weapon:None}).unwrap();
    let m=w.entity(EntityId(1)).unwrap().weapons.iter().position(|m|m.weapon=="cn_pj45a").unwrap();
    assert_eq!(harpoon_core::rules::expanded_damage2026::physical_weapon_mounts(&w,EntityId(1),m).unwrap(),vec![0]);
    harpoon_core::sim::tick(&mut w,&[]);
    assert!(w.rules.aaw.as_ref().unwrap().surface.gunnery.illumination.is_empty());
    let shot=Command::Fire{unit:EntityId(1),contact,weapon:"cn_pj45a".into(),rounds:1};
    assert!(shot.validate_source(&w,Side::Blue).is_ok(),"intact mount 0 remains available");
}

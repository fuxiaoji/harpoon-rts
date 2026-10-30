//! Source-derived candidates only. Root must register and witness real results.
//! Loss setup is the explicitly adjudicated production sink_by boundary; it
//! does not claim a complete weapon engagement or author survivors/dice/HP.
use harpoon_core::rules::expanded_damage2026 as ex;
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};
use std::sync::OnceLock;

#[derive(Clone)]
struct Options {
    seed: u64,
    crew: u32,
    capacity: u32,
    sea: usize,
    look_x: f64,
    lost_x: f64,
    speed: f64,
    current: Vec<f64>,
    radius_m: f64,
}
impl Default for Options {
    fn default() -> Self {
        Self { seed: 1, crew: 90, capacity: 100, sea: 0, look_x: 0.,
            lost_x: 0.2, speed: 0., current: vec![], radius_m: 500. }
    }
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn fixture(o: &Options) -> World {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/../../database/consolidated.json"))).unwrap();
    db.platforms.get_mut("us_ford").unwrap().sensors = vec!["visual".into()];
    db.platforms.get_mut("us_burke_2a").unwrap().sensors.clear();
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"sar-continuation-20261026", "name":"scenario_assumption: source SAR consumers",
        "seed":o.seed,"duration_s":100000,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4",
            "flight_environment":{"wind_from_deg":270,"wind_kts":3},
            "radar_environment":{"sea_state":o.sea,"precipitation":"None"},
            "expanded_damage_2026":{"source":"scenario_assumption: explicit complements and laboratory ship station only",
                "optional":{"search_and_rescue":true},"survivor_current_headings_deg":o.current,
                "units":{
                    "LOOK":{"source":"scenario_assumption: experimental crew and survivor capacity, not actual Ford fit",
                        "crew":o.capacity,"rescue":{"source":"scenario_assumption: explicit recovery station reach; not a book hardware value",
                            "kind":"ship","capacity":o.capacity,"station_radius_m":o.radius_m}},
                    "LOST":{"source":"scenario_assumption: declared complement for actual source loss dice, not an actual Burke complement","crew":o.crew}}},
            "detection_2026":{"environment":{"source":"scenario_assumption: clear daylight open water; declared sea state",
                "sea_state":o.sea,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,
                "water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
        "units":[
            {"side":"Blue","class":"us_ford","name":"LOOK","pos":[o.look_x,0],"course_deg":90,"speed_kts":o.speed,"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"LOST","pos":[o.lost_x,0],"course_deg":0,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"DUMMY","pos":[2000,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let lost = id(&w,"LOST");
    harpoon_core::damage::sink_by(&mut w,lost,Side::Blue);
    assert_eq!(w.rules.expanded_damage.as_ref().unwrap().survivors.len(),1);
    assert!(ex::survivor_view(&w,id(&w,"LOOK")).is_empty());
    w
}
fn group(w: &World) -> (&String, &ex::Survivors) {
    w.rules.expanded_damage.as_ref().unwrap().survivors.iter().next().unwrap()
}
fn source_one_die_seed() -> u64 {
    static SEED: OnceLock<u64> = OnceLock::new();
    *SEED.get_or_init(|| {
        // Select a reproducible source outcome by running actual sink_by. No
        // random result is changed. The same source identity is then reused.
        (1..=128).find(|seed| {
            let w=fixture(&Options {seed:*seed,..Options::default()});
            group(&w).1.casualty_d6==Some(1)
        }).expect("native D6=1 seed within the declared bounded search")
    })
}
fn people(n: u32) -> Options {
    // LOST is >250yd from the stationary LOOK, so no alongside multiplier.
    Options {seed:source_one_die_seed(),crew:n*10,..Options::default()}
}
fn step(w: &mut World) {harpoon_core::sim::tick(w,&[]);}
fn to(w: &mut World,tick: i64) {
    while w.tick<tick {assert!(!w.over,"fixture ended before required source clock");step(w);}
}
fn command(w: &mut World,action: Value) {
    let unit=id(w,"LOOK");
    let cmd: Command=serde_json::from_value(json!({"cmd":"search_and_rescue","unit":unit,"action":action})).unwrap();
    assert!(cmd.validate_source(w,Side::Blue).is_ok(),"public command source precondition");
    harpoon_core::sim::tick(w,&[(Side::Blue,cmd)]);
}
fn begin(w: &mut World,automatic: bool) -> i64 {
    let reports=ex::survivor_view(w,id(w,"LOOK"));
    assert_eq!(reports.len(),1,"actual native Visual discovery is a prerequisite");
    command(w,json!({"type":"begin","group":reports[0]["label"],"automatic":automatic}));
    w.rules.expanded_damage.as_ref().unwrap().rescues[&id(w,"LOOK")].ordered_at
}
fn aboard(w: &World) -> u32 {
    w.rules.expanded_damage.as_ref().unwrap().units.get(&id(w,"LOOK")).map_or(0,|u|u.survivors_aboard)
}
fn recovery_rolls(w: &World) -> Vec<u8> {
    w.rules.expanded_dice.get(&id(w,"LOOK")).into_iter().flatten()
        .filter(|d|d["purpose"]=="ship_recovery_groups")
        .map(|d|d["roll"].as_u64().unwrap() as u8).collect()
}
fn native_scan(w: &World) -> Value {
    w.events.iter().rev().find_map(|e|match &e.kind {
        harpoon_core::events::EventKind::RulesEvent{rule,detail,..} if rule=="14.8.native_visual_scan" =>
            serde_json::from_str::<Value>(detail).ok().map(|v|v["result"].clone()),
        _=>None
    }).expect("real native SAR scan trace")
}

#[test]
fn c01_native_visual_uses_every_unambiguous_source_people_class() {
    for (n,size,signature,category) in [
        (9,6,4,"less_than_ten"),(10,4,3,"ten_to_thirty"),(29,4,3,"ten_to_thirty"),
        (31,3,2,"thirty_to_one_hundred"),(99,3,2,"thirty_to_one_hundred"),
        (101,2,2,"one_hundred_or_more")
    ] {
        let mut w=fixture(&people(n));
        assert_eq!(group(&w).1.count,n,"actual source D6 and original declared crew");
        step(&mut w);
        let own=ex::survivor_view(&w,id(&w,"LOOK"));
        assert_eq!(own.len(),1,"native discovery of source count {n}");
        assert_eq!(own[0]["estimated_group_size"],category);
        let scan=native_scan(&w);
        assert_eq!(scan["physical_size"],size);
        assert_eq!(scan["signature"],signature);
        assert_eq!(scan["no_wake_multiplier"],0.5);
        assert!(w.sides[0].contacts.is_empty(),"SAR never synthesizes a combat Contact");
        for private in ["count","source_crew","lost_unit","casualty_d6","drift","sightings"] {
            assert!(own[0].get(private).is_none(),"private {private}");
        }
    }
}

#[test]
fn c02_printed_people_endpoint_overlap_is_source_pending_not_guessed() {
    for n in [30,100] {
        let mut w=fixture(&people(n));
        assert_eq!(group(&w).1.count,n);
        let key=group(&w).0.clone();
        assert!(ex::survivor_sighting_profile(&w,&key,false).is_err(),"endpoint {n} must not pick one printed overlapping class");
        step(&mut w);
        assert!(ex::survivor_view(&w,id(&w,"LOOK")).is_empty());
        assert!(native_scan(&w).get("blocked").is_some());
    }
}

#[test]
fn c03_public_manual_recovery_obeys_work_delay_real_capacity_and_same_turn_idempotence() {
    let mut o=people(9);o.capacity=4;
    let mut w=fixture(&o);step(&mut w);let ordered=begin(&mut w,false);
    to(&mut w,ordered+178);
    command(&mut w,json!({"type":"recover_due"}));
    assert_eq!(aboard(&w),0,"179 elapsed seconds cannot perform one Tactical Turn of work");
    command(&mut w,json!({"type":"recover_due"}));
    assert_eq!(aboard(&w),4,"source minimum5 people is capped by the declared four-person capacity");
    assert_eq!(group(&w).1.rescued,4);assert_eq!(group(&w).1.count,9);
    let rolls=recovery_rolls(&w);assert_eq!(rolls.len(),1);
    command(&mut w,json!({"type":"recover_due"}));
    assert_eq!(aboard(&w),4);assert_eq!(recovery_rolls(&w),rolls);
    let mut restored: World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let end=w.tick+365;to(&mut w,end);to(&mut restored,end);
    command(&mut w,json!({"type":"recover_due"}));
    command(&mut restored,json!({"type":"recover_due"}));
    assert_eq!(aboard(&w),4,"new turn and reload cannot exceed original complement capacity");
    assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap());
}

#[test]
fn c04_actual_recovery_d6_is_reduced_by_each_sea_state_above_two() {
    let mut calm=people(200);calm.capacity=100;
    let mut rough=calm.clone();rough.sea=4;
    let mut a=fixture(&calm);let mut b=fixture(&rough);
    step(&mut a);step(&mut b);let at=begin(&mut a,false);assert_eq!(begin(&mut b,false),at);
    to(&mut a,at+180);to(&mut b,at+180);
    command(&mut a,json!({"type":"recover_due"}));command(&mut b,json!({"type":"recover_due"}));
    let ar=recovery_rolls(&a);let br=recovery_rolls(&b);
    assert_eq!(ar.len(),1);assert_eq!(br,ar,"matched physical identity retains the same native source D6");
    let d=u32::from(ar[0]);
    assert_eq!(aboard(&a),d*5,"D6/2 groups of10; preserve fractional groups until people");
    assert_eq!(aboard(&b),d.saturating_sub(2)*5,"SS4 subtracts2 before division");
}

#[test]
fn c05_actual_automatic_recovery_requires_stationary_and_upwind() {
    for moving in [false,true] {
        let mut o=people(9);
        if moving {o.speed=0.01;}else{o.look_x=0.4;}
        let mut w=fixture(&o);step(&mut w);begin(&mut w,true);to(&mut w,540);
        assert_eq!(aboard(&w),0);assert_eq!(group(&w).1.rescued,0);
        let task=&w.rules.expanded_damage.as_ref().unwrap().rescues[&id(&w,"LOOK")];
        let pending=task.pending.as_deref().expect("actual recovery station failure is retained");
        if moving {assert!(w.entity(id(&w,"LOOK")).unwrap().speed_kts>0.);assert!(pending.contains("stationary"));}
        else {assert!(pending.contains("upwind"));}
        assert!(recovery_rolls(&w).is_empty(),"failed station precondition does not spend source recovery dice");
    }
}

#[test]
fn c06_partial_actual_recovery_restarts_with_identical_world_and_no_same_turn_second_roll() {
    let mut w=fixture(&people(200));step(&mut w);let at=begin(&mut w,false);to(&mut w,at+179);
    let mut restored: World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for x in [&mut w,&mut restored] {
        command(x,json!({"type":"recover_due"}));
        assert!((5..=30).contains(&aboard(x)));assert!(group(x).1.rescued<group(x).1.count);
        let before=aboard(x);let rolls=recovery_rolls(x);
        command(x,json!({"type":"recover_due"}));assert_eq!(aboard(x),before);assert_eq!(recovery_rolls(x),rolls);
    }
    assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap());
}

#[test]
fn c07_real_six_hour_current_and_native_visual_reacquisition_restore_exactly() {
    let mut o=people(9);o.current=vec![90.];
    let mut w=fixture(&o);step(&mut w);let initial=group(&w).1.position;
    to(&mut w,21599);
    assert_eq!(group(&w).1.position,initial);
    let mut restored: World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    to(&mut w,21660);to(&mut restored,21660);
    let g=group(&w).1;let (speed,heading)=g.drift.expect("declared geographic heading starts native source current");
    assert!((1.0..=6.0).contains(&speed));assert_eq!(heading,90.);
    assert!(g.position.x>initial.x);assert!((g.position.x-initial.x-speed*60./3600.).abs()<1e-9);
    let reports=ex::survivor_view(&w,id(&w,"LOOK"));assert_eq!(reports.len(),1);
    assert_eq!(reports[0]["observed_at"],21660,"a real Visual scan must see the moved physical point after six hours");
    assert_eq!(reports[0]["position"],serde_json::to_value(g.position).unwrap());
    assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap());
}

#[test]
fn c08_unknown_geographic_current_cannot_be_substituted_with_a_stationary_live_point() {
    let mut w=fixture(&people(9));step(&mut w);to(&mut w,21600);
    let before=ex::survivor_view(&w,id(&w,"LOOK"));
    to(&mut w,21660);
    assert_eq!(ex::survivor_view(&w,id(&w,"LOOK")),before,"unknown drift must retain the last measured snapshot");
    assert!(group(&w).1.drift.is_none());
    assert!(native_scan(&w).get("blocked").is_some());
}

#[test]
fn c09_order_time_away_from_the_station_is_not_continuous_rescue_work() {
    let mut o=people(9);o.lost_x=0.5;
    let mut w=fixture(&o);step(&mut w);begin(&mut w,false);to(&mut w,200);
    let own=id(&w,"LOOK");
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::SetCourse{unit:own,course_deg:90.}),
        (Side::Blue,Command::SetSpeed{unit:own,speed_kts:10.})]);
    while w.entity(own).unwrap().pos.x<0.25 {assert!(w.tick<1800,"normal source movement must reach the station");step(&mut w);}
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,Command::Stop{unit:own})]);
    while w.entity(own).unwrap().speed_kts>1e-9 {assert!(w.tick<2000);step(&mut w);}
    let arrival=w.tick;
    let e=w.entity(own).unwrap();let g=group(&w).1;
    assert!(e.pos.range_to(g.position)*1852.<o.radius_m && e.pos.x<g.position.x,"actual stationary upwind station prerequisite");
    command(&mut w,json!({"type":"recover_due"}));
    assert_eq!(aboard(&w),0,"200s at the remote order point cannot replace180s actual rescue work");
    assert!(recovery_rolls(&w).is_empty());
    let mut restored: World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    to(&mut w,arrival+181);to(&mut restored,arrival+181);
    command(&mut w,json!({"type":"recover_due"}));command(&mut restored,json!({"type":"recover_due"}));
    assert!(aboard(&w)>0,"after actual continuous source work the public command performs real recovery");
    assert!(w.rules.expanded_damage.as_ref().unwrap().rescues[&own].pending.is_none(),
        "actual successful station work clears a previous off-station pending message");
    assert_eq!(serde_json::to_vec(&w).unwrap(),serde_json::to_vec(&restored).unwrap());
}

#[test]
fn c10_drift_begins_strictly_after_six_hours_and_unknown_current_freezes_the_last_legal_measurement() {
    for declared in [true,false] {
        let mut o=people(9);
        if declared {o.current=vec![90.];}
        let mut w=fixture(&o);step(&mut w);let initial=group(&w).1.position;
        to(&mut w,21600);
        assert!(group(&w).1.drift.is_none(),"more than six hours does not include the exact21600s endpoint");
        assert_eq!(group(&w).1.position,initial);
        let at_boundary=ex::survivor_view(&w,id(&w,"LOOK"));
        assert_eq!(at_boundary[0]["observed_at"],21600,"the last stationary source point remains measurable at exactly six hours");
        step(&mut w);
        let after=ex::survivor_view(&w,id(&w,"LOOK"));
        if declared {
            let (speed,heading)=group(&w).1.drift.expect("first post-six-hour tick creates the actual source current");
            assert_eq!(heading,90.);
            assert!((group(&w).1.position.x-initial.x-speed/3600.).abs()<1e-9);
            assert_eq!(after[0]["observed_at"],21601);
        } else {
            assert!(group(&w).1.drift.is_none());
            assert_eq!(after,at_boundary,"unknown current preserves the exact last legal snapshot at21600");
        }
    }
}

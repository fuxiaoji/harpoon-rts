//! Source-only §8.5.4.3 candidates: English2026 PDF101/8-21. Three
//! independent original bodies carry expressly authored laboratory DC fits.
//! Native located reports and actual physical stock are required; no contact,
//! tracking quality, random die, HP or private damage result is injected.
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};
const DC: &str = "lab_dc_safety";
const SONAR: &str = "lab_dc_safety_sonar";
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn fixture(speed: f64) -> World {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut weapon = db.weapons["cw_depth_charge"].clone();
    weapon.id = DC.into();
    weapon.name =
        "scenario_assumption: source safety algorithm stores, no real Ford depth-charge fit".into();
    weapon.pk = 0.;
    weapon.damage = 999.;
    weapon.reload_s = 9000.;
    db.weapons.insert(DC.into(), weapon);
    let mut mount = db.platforms["us_ford"].weapons[0].clone();
    mount.weapon = DC.into();
    mount.mounts = 4;
    mount.ammo = 19;
    db.platforms.get_mut("us_ford").unwrap().weapons = vec![mount];
    let mut sonar = db.sensors.values().next().unwrap().clone();
    sonar.id = SONAR.into();
    sonar.kind = harpoon_core::db::SensorKind::SonarActive;
    sonar.sub_range_nm = 16.;
    sonar.name = "scenario_assumption: declared Gen5 laboratory bow sonar".into();
    db.sensors.insert(SONAR.into(), sonar);
    let mut director = db.platforms["us_ford"].clone();
    director.id = "lab_dc_safety_observer".into();
    director.name = "scenario_assumption: independent stationary native sonar director".into();
    director.sensors = vec![SONAR.into()];
    director.weapons.clear();
    db.platforms.insert(director.id.clone(), director);
    db.platforms.get_mut("us_ford").unwrap().sensors.clear();
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    db.platforms.get_mut("cn_093").unwrap().weapons.clear();
    let unit_fit = json!({"source":"scenario_assumption: two9-charge rails/two4-charge projectors and50 reserve on each original sizeA body","mount_index":0,"physical_size":0,"reserve":50,"mounts":[{"physical":0,"kind":"rail","capacity":9,"loaded":9},{"physical":1,"kind":"rail","capacity":9,"loaded":9},{"physical":2,"kind":"projector","capacity":4,"loaded":4},{"physical":3,"kind":"projector","capacity":4,"loaded":4}]});
    let sc:Scenario=serde_json::from_value(json!({"id":"dc-safety-source","name":"scenario_assumption: original bodies with explicit experimental DC safety fit","seed":20261111,"duration_s":1000,"weather":{"visibility_nm":0,"jam":[0,0]},
      "context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
        "asw_weapons_2026":{"source":"scenario_assumption: source DC safety experiment","standoffs":{},"depth_charges":{DC:{"source":"scenario_assumption: experimental Major7/Minor3DP, not an Annex claim","damage_major":7.,"damage_minor":3.,"max_depth_band":7,"units":{"LAB1":unit_fit.clone(),"LAB2":unit_fit.clone(),"LAB3":unit_fit}}}},
        "communications_2026":{"source":"scenario_assumption: actual directed voice route, no free fused location","units":{
          "DIRECTOR":{"source":"scenario_assumption","fits":{"voice":{"source":"scenario_assumption","protocol":"dc-safety-voice","send":true,"receive":true,"operational":true,"peers":["LAB1","LAB2","LAB3"],"warning_kind":"voice","transport":{"type":"hf","max_range_nm":20}}}},
          "LAB1":{"source":"scenario_assumption","fits":{"voice":{"source":"scenario_assumption","protocol":"dc-safety-voice","send":false,"receive":true,"operational":true,"peers":[],"warning_kind":"voice","transport":{"type":"hf","max_range_nm":20}}}},
          "LAB2":{"source":"scenario_assumption","fits":{"voice":{"source":"scenario_assumption","protocol":"dc-safety-voice","send":false,"receive":true,"operational":true,"peers":[],"warning_kind":"voice","transport":{"type":"hf","max_range_nm":20}}}},
          "LAB3":{"source":"scenario_assumption","fits":{"voice":{"source":"scenario_assumption","protocol":"dc-safety-voice","send":false,"receive":true,"operational":true,"peers":[],"warning_kind":"voice","transport":{"type":"hf","max_range_nm":20}}}}}},
        "detection_2026":{"bindings":{format!("lab_dc_safety_observer:{SONAR}"):{"type":"sonar","source":"scenario_assumption: independent actual Gen5 LMF bow receiver","generation":5,"passive_range_nm":null,"active_range_nm":16.,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false}},
          "environment":{"source":"scenario_assumption: explicit open-water geometry","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
          "targets":{"cn_093":{"source":"scenario_assumption: explicit controlled acoustic target","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
      "units":[{"side":"Blue","class":"us_ford","name":"LAB1","pos":[-0.12,0],"course_deg":90,"speed_kts":speed,"fire_on":false},
        {"side":"Blue","class":"us_ford","name":"LAB2","pos":[0.06,0.10392304845413264],"course_deg":210,"speed_kts":speed,"fire_on":false},
        {"side":"Blue","class":"us_ford","name":"LAB3","pos":[0.06,-0.10392304845413264],"course_deg":330,"speed_kts":speed,"fire_on":false},
        {"side":"Blue","class":"lab_dc_safety_observer","name":"DIRECTOR","pos":[0,1],"course_deg":180,"speed_kts":0,"fire_on":false},
        {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let director = id(&w, "DIRECTOR");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSonar {
                unit: director,
                active: true,
            },
        )],
    );
    let target = id(&w, "TARGET");
    let held = |world: &World| {
        world
            .rules
            .aaw
            .as_ref()
            .unwrap()
            .firecontrol
            .tracks
            .values()
            .any(|t| {
                t.observer == director
                    && t.target == target
                    && t.report
                        .as_ref()
                        .is_some_and(|r| r.measured_position.is_some())
            })
    };
    while w.tick < 360 && !held(&w) {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    let scans: Vec<_> = w
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule, unit, detail, ..
            } if rule == "5.scan" && *unit == Some(director) => Some(detail.clone()),
            _ => None,
        })
        .take(3)
        .collect();
    assert!(
        held(&w),
        "independent real native sender did not acquire a measured report: {scans:?}"
    );
    let receivers: Vec<_> = ["LAB1", "LAB2", "LAB3"]
        .into_iter()
        .map(|name| id(&w, name))
        .collect();
    let send:Command=serde_json::from_value(json!({"cmd":"communications","unit":director,"action":{"type":"send_warning","contact":contact(&w),"sensor":SONAR,"fit":"voice","receivers":receivers}})).unwrap();
    send.validate_source(&w, Side::Blue)
        .expect("actual sender native report and operational directed transport");
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, send)]);
    for receiver in &receivers {
        let report =
            harpoon_core::rules::communications2026::received_located_report(&w, *receiver, target)
                .unwrap()
                .expect("all actual recipients must receive the original raw located report");
        assert_eq!(report.observer, director);
        assert!(report.measured_position.is_some());
        assert!(
            !w.rules
                .aaw
                .as_ref()
                .unwrap()
                .firecontrol
                .tracks
                .values()
                .any(|t| t.observer == *receiver && t.target == target),
            "voice receipt cannot manufacture a receiver's own native sonar track"
        );
    }
    let evasion: Command = serde_json::from_value(
        json!({"cmd":"asw","unit":target,"action":{"type":"depth_charge_evasion","evading":true}}),
    )
    .unwrap();
    evasion
        .validate_source(&w, Side::Red)
        .expect("actual explicit defender declaration");
    harpoon_core::sim::tick(&mut w, &[(Side::Red, evasion)]);
    w
}
fn contact(w: &World) -> ContactId {
    w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == id(w, "TARGET") && !c.stale && !c.bearing_only)
        .unwrap()
        .id
}
fn pattern(w: &World, actor: EntityId, band: usize) -> Command {
    let command:Command=serde_json::from_value(json!({"cmd":"asw","unit":actor,"action":{"type":"depth_charge_pattern","contact":contact(w),"weapon":DC,"charges":12,"depth_band":band,"distribution":"straight","allocations":[{"physical":0,"charges":6},{"physical":1,"charges":6}]}})).unwrap();
    command.validate_source(w, Side::Blue).expect(
        "actual source-qualified received-position pattern admission, not a truth/stock bypass",
    );
    command
}
fn finish_depth_dialogue(w: &mut World, actors: &[(EntityId, usize)]) {
    let target = id(w, "TARGET");
    let view = harpoon_core::rules::asw_weapons2026::depth_charge_own_view(w, target);
    let incoming = view["incoming_patterns"]
        .as_array()
        .expect("actual defender's own incoming pattern aliases");
    let reports:Vec<_>=incoming.iter().filter(|p|p["depth_report_required"]==true).map(|p|{
        let band=p["own_depth_band_at_pass"].as_u64().unwrap() as usize;let pair=if band<7{[band,band+1]}else{[band-1,band]};
        let c:Command=serde_json::from_value(json!({"cmd":"asw","unit":target,"action":{"type":"depth_charge_depth_report","attack":p["attack"],"bands":pair}})).unwrap();
        c.validate_source(w,Side::Red).expect("real defender depth choice from own original pass observation");(Side::Red,c)
    }).collect();
    assert_eq!(
        reports.len(),
        actors.len(),
        "every uncertain incoming pattern needs the real source depth dialogue"
    );
    harpoon_core::sim::tick(w, &reports);
    let selects:Vec<_>=actors.iter().map(|(actor,band)|{
        let own=harpoon_core::rules::asw_weapons2026::depth_charge_own_view(w,*actor);let attack=own["pending_attacks"].as_array().unwrap().iter().find(|p|p["select_depth_required"]==true).unwrap()["attack"].clone();
        let c:Command=serde_json::from_value(json!({"cmd":"asw","unit":actor,"action":{"type":"depth_charge_select_depth","attack":attack,"depth_band":band}})).unwrap();
        c.validate_source(w,Side::Blue).expect("original attacker selects only after actual depth receipt");(Side::Blue,c)
    }).collect();
    harpoon_core::sim::tick(w, &selects);
}
fn hits_since(w: &World, actor: EntityId, since: u64) -> Vec<Value> {
    w.events
        .iter()
        .filter(|e| e.seq >= since)
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule, unit, detail, ..
            } if rule == "hit_resolved" && *unit == Some(actor) => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn assert_one_underwater_major(w: &World, actor: EntityId, since: u64) {
    let hits = hits_since(w, actor, since);
    assert_eq!(
        hits.len(),
        1,
        "one physical own-ship safety hit must join the real common batch"
    );
    assert_eq!(hits[0]["dp"].as_f64(),Some(7.),"the source specifies one original charge Major DP, not pattern count, Pk999 or TPS-reduced surrogate");
    assert_eq!(
        hits[0]["column"], "underwater",
        "self-blast criticals use the true common Underwater column"
    );
}
#[test]
fn dcs01_shallow_low_speed_spends_pattern_and_commits_one_major_own_hit_but_exactly15_is_safe() {
    for (speed, band) in [(0., 1), (0., 2), (15., 1), (15., 2)] {
        let mut w = fixture(speed);
        let actor = id(&w, "LAB1");
        let hp = w.entity(actor).unwrap().hp;
        let stock = w.entity(actor).unwrap().weapons[0].ammo;
        let since = w.event_seq;
        let c = pattern(&w, actor, band);
        harpoon_core::sim::tick(&mut w, &[(Side::Blue, c)]);
        finish_depth_dialogue(&mut w, &[(actor, band)]);
        assert_eq!(w.entity(actor).unwrap().weapons[0].ammo, stock - 12);
        if speed < 15. {
            assert!(w.entity(actor).unwrap().hp<=hp-7.,"8.5.4.3 automatic single-charge Major self-damage cannot depend on whether the submarine was hit");
            assert_one_underwater_major(&w, actor, since);
        } else {
            assert_eq!(
                w.entity(actor).unwrap().hp,
                hp,
                "source requires at least15kt; equality is safe for the first attacking ship"
            );
            assert!(hits_since(&w, actor, since).is_empty());
        }
    }
}
#[test]
fn dcs02_known_first_ship_then_two_same_turn_followers_receive_one_major_each_independent_of_client_order(
) {
    let mut before = fixture(15.);
    let first = id(&before, "LAB1");
    let second = id(&before, "LAB2");
    let third = id(&before, "LAB3");
    let first_hp = before.entity(first).unwrap().hp;
    let c = pattern(&before, first, 1);
    harpoon_core::sim::tick(&mut before, &[(Side::Blue, c)]);
    finish_depth_dialogue(&mut before, &[(first, 1)]);
    assert_eq!(before.entity(first).unwrap().hp, first_hp);
    let mut branches: Vec<World> = (0..2)
        .map(|_| serde_json::from_value(serde_json::to_value(&before).unwrap()).unwrap())
        .collect();
    for (reverse, w) in branches.iter_mut().enumerate() {
        let since = w.event_seq;
        let hp2 = w.entity(second).unwrap().hp;
        let hp3 = w.entity(third).unwrap().hp;
        let ammo2 = w.entity(second).unwrap().weapons[0].ammo;
        let ammo3 = w.entity(third).unwrap().weapons[0].ammo;
        let mut commands = vec![
            (Side::Blue, pattern(w, second, 1)),
            (Side::Blue, pattern(w, third, 1)),
        ];
        if reverse == 1 {
            commands.reverse();
        }
        harpoon_core::sim::tick(w, &commands);
        finish_depth_dialogue(w, &[(second, 1), (third, 1)]);
        assert_eq!(
            w.entity(first).unwrap().hp,
            first_hp,
            "the already known first ship is not a lexicographic/network-order casualty"
        );
        assert!(w.entity(second).unwrap().hp<=hp2-7.&&w.entity(third).unwrap().hp<=hp3-7.,"8.5.4.3 each later ship entering the first pattern's Tactical Turn blast suffers one charge Major damage");
        assert_eq!(w.entity(second).unwrap().weapons[0].ammo, ammo2 - 12);
        assert_eq!(w.entity(third).unwrap().weapons[0].ammo, ammo3 - 12);
        assert_one_underwater_major(w, second, since);
        assert_one_underwater_major(w, third, since);
    }
    for actor in [first, second, third] {
        assert_eq!(
            branches[0].entity(actor).unwrap().hp,
            branches[1].entity(actor).unwrap().hp
        );
        assert_eq!(
            serde_json::to_value(&branches[0].entity(actor).unwrap().weapons).unwrap(),
            serde_json::to_value(&branches[1].entity(actor).unwrap().weapons).unwrap()
        );
    }
}

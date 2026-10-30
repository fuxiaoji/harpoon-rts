//! Source-only14.1.8 nuclear2psi actual mission-abort -> direct aerial ASW consumer.
//! Reuses the accepted source air-DC/native report fixture and explicit laboratory
//! nuclear delivery/AAW terminal APIs; no runtime flag, HP or die is inserted.
//! Existing air-DC source: English Jan2026 PDF101/8-21 section8.5.4.4;
//! PDF109/9-8 section9.7.2 refers back to that section. Every aircraft store,
//! damage value, mass-neutral load and release geometry below is explicitly
//! laboratory policy, not a claim about MH60R or any real Annex DC fit.
use harpoon_core::rules::{asw_weapons2026 as asw, communications2026 as comms};
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};
const DC: &str = "lab_air_dc";
const SONAR: &str = "lab_air_dc_sonar";

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn cmd(actor: EntityId, domain: &str, action: Value) -> Command {
    serde_json::from_value(json!({"cmd":domain,"unit":actor,"action":action}))
        .expect("future typed action through the existing command envelope")
}
fn issue(w: &mut World, side: Side, actor: EntityId, domain: &str, action: Value) {
    let command = cmd(actor, domain, action);
    command
        .validate_source(w, side)
        .expect("real source/own knowledge admission");
    let since = w.event_seq;
    harpoon_core::sim::tick(w, &[(side, command)]);
    let rejected: Vec<_> = w
        .events
        .iter()
        .filter(|e| e.seq > since)
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. }
                if rule == "8.5.4.4.request_rejected" || rule == "8.5.4.request_rejected" =>
            {
                Some((rule, detail))
            }
            _ => None,
        })
        .collect();
    assert!(
        rejected.is_empty(),
        "real post-movement ASW rejection: {rejected:?}"
    );
}
fn contact(w: &World) -> ContactId {
    w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == id(w, "TARGET") && !c.stale && !c.bearing_only)
        .unwrap()
        .id
}
fn own(w: &World, actor: EntityId) -> Value {
    asw::depth_charge_own_view(w, actor)
}
fn ammo(w: &World, actor: EntityId) -> u16 {
    w.entity(actor)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == DC)
        .unwrap()
        .ammo
}
fn result(w: &World) -> Value {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == "8.5.4.4.pattern_result" => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .last()
        .expect("actual typed air-depth-charge common-batch trace")
}
fn fixture(payload: Value, dipping: bool) -> World {
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let mut weapon = db.weapons["cw_depth_charge"].clone();
    weapon.id = DC.into();
    weapon.name =
        "scenario_assumption: laboratory airborne store; no real aircraft/DC fitting claim".into();
    weapon.pk = 0.;
    weapon.damage = 999.;
    weapon.reload_s = 9000.;
    db.weapons.insert(DC.into(), weapon);
    let mut mount = db.platforms["us_mh60r"].weapons[0].clone();
    mount.weapon = DC.into();
    mount.mounts = 1;
    mount.ammo = 24;
    db.platforms.get_mut("us_mh60r").unwrap().weapons = vec![mount];
    db.platforms.get_mut("us_mh60r").unwrap().sensors =
        if dipping { vec![SONAR.into()] } else { vec![] };
    let mut sonar = db.sensors.values().next().unwrap().clone();
    sonar.id = SONAR.into();
    sonar.kind = harpoon_core::db::SensorKind::SonarActive;
    sonar.name = "scenario_assumption: controlled laboratory receiver".into();
    sonar.sub_range_nm = 16.;
    db.sensors.insert(SONAR.into(), sonar);
    db.platforms
        .get_mut("us_ford")
        .unwrap()
        .sensors
        .push(SONAR.into());
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    let mut scene = json!({"id":"air-dc-source-candidate","name":"scenario_assumption: laboratory algorithm only, real Annex applicability unresolved","seed":20261104,"duration_s":20000,
      "weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4","radar_environment":{"sea_state":0,"precipitation":"None"},
        "nuclear_2026":{"source":"scenario_assumption: qualified source-table laboratory delivery, no nuclear ESSM Annex fit claim","units":{"NUCLEAR_LAB":{"us_essm_i":{"source":"scenario_assumption: existing nuclear lifecycle laboratory carrier","source_variant":"scenario_assumption:air_abort_consumer_lab","allowed_yields_kt":[1],"selected_yield_kt":1,"medium":"surface","fuze":"free_fall","authorized":true}}}},"aviation_2026":{"source":"scenario_assumption: explicit mass-neutral mechanism fixture, not operational aircraft stores","loads":{"us_mh60r":{"payload_kg":0,"tanks":[],"source":"scenario_assumption: mass-neutral experimental DC mechanism case"}},"parking":{},"initial_fuel_nm":{"AIR":200,"NUCLEAR_TRIGGER":200}},
        "asw_weapons_2026":{"source":"scenario_assumption: explicit laboratory air-DC hardware","standoffs":{},"air_depth_charges":{DC:{
          "source":"scenario_assumption: independent laboratory store, no Annex interpretation","payload":payload,"damage_major":7.,"damage_minor":3.,"max_depth_band":7,
          "units":{"AIR":{"source":"scenario_assumption: original helicopter body carrying laboratory stores","mount_index":0,"round_mass_kg":0.,
            "release_policy":{"source":"scenario_assumption: experiment geometry; no 9.7.1 torpedo envelope inherited","maximum_datum_range_nm":2.}}}}}},
        "communications_2026":{"source":"scenario_assumption: directed physical voice route","units":{
          "OBSERVER":{"source":"scenario_assumption","fits":{"voice":{"source":"scenario_assumption","protocol":"lab-air-dc","send":true,"receive":true,"operational":true,"peers":["AIR"],"warning_kind":"voice","transport":{"type":"hf","max_range_nm":20}}}},
          "AIR":{"source":"scenario_assumption","fits":{"voice":{"source":"scenario_assumption","protocol":"lab-air-dc","send":true,"receive":true,"operational":true,"peers":[],"warning_kind":"voice","transport":{"type":"hf","max_range_nm":20}}}}}},
        "ew_2026":{"fits":{"lab_acm":{"source":"scenario_assumption: explicit original-body acoustic CM experiment","kind":"torpedo_acm","generation":1,"launcher":"internal","endurance_s":null,"deployment_s":null,"radar_height_m":null}},"loadouts":{"cn_093":{"lab_acm":1}},"environment":null},
        "detection_2026":{"bindings":{format!("us_ford:{SONAR}"):{"type":"sonar","source":"scenario_assumption: independent Gen5 bow receiver","generation":5,"passive_range_nm":null,"active_range_nm":16.,"frequency":"lmf","mount":"bow","active_torpedo_detection":false,"high_speed_tow":false}},
          "environment":{"source":"scenario_assumption: controlled open-water target","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
          "targets":{"cn_093":{"source":"scenario_assumption: explicit laboratory acoustic attributes","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}},
      "units":[{"side":"Blue","class":"us_ford","name":"OBSERVER","pos":[0,1],"course_deg":180,"speed_kts":0,"sonar_active":true,"fire_on":false},
        {"side":"Blue","class":"us_mh60r","name":"AIR","pos":[0,0],"speed_kts":0,"altitude_m":if dipping{10}else{1000},"fire_on":false},
        {"side":"Red","class":"cn_093","name":"TARGET","pos":[0.08,0],"speed_kts":0,"depth_m":50,"fire_on":false},{"side":"Blue","class":"us_ford","name":"NUCLEAR_LAB","pos":[0.35,0],"speed_kts":0,"fire_on":false},{"side":"Red","class":"us_mh60r","name":"NUCLEAR_TRIGGER","pos":[0.35,0.01],"speed_kts":0,"altitude_m":1000,"fire_on":false}]});
    if dipping {
        scene["context"]["detection_2026"]["bindings"][format!("us_mh60r:{SONAR}")] = json!({"type":"sonar","source":"scenario_assumption: actually deployed Gen4 active dipping receiver","generation":4,"passive_range_nm":null,"active_range_nm":16.,"frequency":"lmf","mount":"dipping","active_torpedo_detection":false,"high_speed_tow":false});
        scene["context"]["detection_2026"]["dipping_platforms"] = json!({"us_mh60r":"helicopter"});
    }
    if matches!(
        scene["context"]["asw_weapons_2026"]["air_depth_charges"][DC]["payload"]["type"].as_str(),
        Some("cluster_rbk100")
    ) {
        scene["context"]["asw_weapons_2026"]["submarine_hulls"] = json!({"TARGET":{"source":"scenario_assumption: laboratory four-class hull target for source contact algorithm; not a real Type093 design claim","size":"medium","design":"double","in_service_year":2000}});
    }
    let sc:Scenario=serde_json::from_value(scene).expect("source red must reach ordinary strict air_depth_charges configuration, not a new Rust enum compile error");
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let air = id(&w, "AIR");
    let trigger = id(&w, "NUCLEAR_TRIGGER");
    issue(
        &mut w,
        Side::Red,
        trigger,
        "aviation",
        json!({"type":"throttle","setting":"Hover"}),
    );
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Red,
            Command::SetAutoChaff {
                unit: trigger,
                on: false,
            },
        )],
    );
    // Actual operator activation is required for custom sensor IDs. The
    // legacy sonar_a* spawn shortcut does not activate this experiment's ID.
    let observer = id(&w, "OBSERVER");
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSonar {
                unit: observer,
                active: true,
            },
        )],
    );
    if dipping {
        harpoon_core::sim::tick(
            &mut w,
            &[(
                Side::Blue,
                Command::SetSonar {
                    unit: air,
                    active: true,
                },
            )],
        );
    }
    issue(
        &mut w,
        Side::Blue,
        air,
        "aviation",
        json!({"type":"throttle","setting":"Hover"}),
    );
    if dipping {
        issue(
            &mut w,
            Side::Blue,
            air,
            "detection",
            json!({"type":"dip","sensor":SONAR,"deploy":true,"depth_m":15.}),
        );
    }
    let target = id(&w, "TARGET");
    while w.tick < 1800 {
        let located = w.sides[0]
            .contacts
            .values()
            .any(|c| c.about == target && !c.stale && !c.bearing_only);
        let own_dip = !dipping
            || w.rules
                .aaw
                .as_ref()
                .unwrap()
                .firecontrol
                .tracks
                .values()
                .any(|t| {
                    t.observer == air
                        && t.target == target
                        && t.report
                            .as_ref()
                            .is_some_and(|r| r.measured_position.is_some())
                });
        if located && own_dip {
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(
        w.sides[0]
            .contacts
            .values()
            .any(|c| c.about == target && !c.stale && !c.bearing_only),
        "the original sensor executor must produce the located contact"
    );
    if dipping {
        assert!(
            w.rules
                .aaw
                .as_ref()
                .unwrap()
                .firecontrol
                .tracks
                .values()
                .any(|t| t.observer == air && t.target == target && t.report.is_some()),
            "the exception needs an actual native deployed dipping report"
        );
    } else {
        let observer = id(&w, "OBSERVER");
        let cid = contact(&w);
        issue(
            &mut w,
            Side::Blue,
            observer,
            "communications",
            json!({"type":"send_warning","contact":cid,"sensor":SONAR,"fit":"voice","receivers":[air]}),
        );
        assert!(
            comms::received_located_report(&w, air, target)
                .unwrap()
                .is_some(),
            "genuine current received location, never free side fusion"
        );
    }
    let target = id(&w, "TARGET");
    issue(
        &mut w,
        Side::Red,
        target,
        "asw",
        json!({"type":"depth_charge_evasion","evading":true}),
    );
    w
}
fn drop(w: &mut World, charges: u16, depth: Option<usize>) {
    let actor = id(w, "AIR");
    let cid = contact(w);
    issue(
        w,
        Side::Blue,
        actor,
        "asw",
        json!({"type":"air_depth_charge_pattern","contact":cid,"weapon":DC,"charges":charges,"depth_band":depth}),
    );
}
fn proposed_drop(w: &World) -> Command {
    cmd(
        id(w, "AIR"),
        "asw",
        json!({"type":"air_depth_charge_pattern","contact":contact(w),"weapon":DC,"charges":1,"depth_band":1}),
    )
}
fn real_abort_fixture() -> World {
    use harpoon_core::rules::{aaw2026 as a, nuclear2026 as n};
    let mut w = fixture(json!({"type":"conventional"}), false);
    proposed_drop(&w).validate_source(&w,Side::Blue).expect("unchanged healthy original aircraft must have genuine native-delivered localization, source Flight and a releasable real store before the nuclear event");
    let shooter = id(&w, "NUCLEAR_LAB");
    let target = id(&w, "NUCLEAR_TRIGGER");
    let air = id(&w, "AIR");
    let actual_speed = w.db.weapons["us_essm_i"].speed_kts;
    let attack = a::MissileAttack {
        ata: 2.,
        maneuver_rating: 0.,
        maneuvering: false,
        speed_kts: actual_speed,
        signature: 2,
        combat_generation: 4,
        automatic: false,
        target_is_missile: false,
        range_nm: 0.01,
        bearing: a::Bearing::Closing,
        terminal_maneuvers: false,
        sea_skimming: false,
        sea_capability: 2,
        noe: false,
        manpads_or_salh: false,
        high_diving: false,
        land_clutter: false,
        radar_guided: true,
        countermeasures: None,
        hoj: false,
    };
    // Author-frozen source terminal parameters, as in the accepted nuclear
    // lifecycle fixture. The source algorithm computes probability and rolls
    // the real physical packet. No private result/flag/damage is supplied.
    let probability = a::missile_solution(&attack, 1).unwrap().probability;
    let mut selected = false;
    for _ in 0..32 {
        // World stores the last allocated number; allocate_id increments it.
        // Read this only to choose an ordinary launch instant, never assign it.
        let next = w
            .next_entity
            .checked_add(1)
            .expect("available actual allocation space");
        let identity = format!("aaw:{}:us_essm_i:{}:{next}", shooter.0, target.0);
        let mut r = w
            .rng
            .derive(harpoon_core::Rng::hash_tag(w.tick, shooter.0, &identity));
        let d = 1 + (r.next_f64() * 100.).floor() as u8;
        if f64::from(d) <= probability * 100. {
            selected = true;
            break;
        }
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert!(
        selected,
        "bounded ordinary launch schedule needs a naturally successful original terminal stream"
    );
    let def = w.db.weapons["us_essm_i"].clone();
    let carrier = harpoon_core::weapons::launch(&mut w, shooter, &def, Some(target), None)
        .expect("actual authorized laboratory nuclear carrier body");
    let body = w
        .entity(carrier)
        .and_then(|e| e.projectile.as_ref())
        .expect("actual returned physical carrier must exist");
    assert_eq!(body.launched_by, shooter);
    assert_eq!(body.target, Some(target));
    assert_eq!(body.weapon, "us_essm_i");
    let budget = a::intercept_budget(
        4,
        false,
        16000.,
        actual_speed,
        68.8,
        1650.,
        3.5,
        false,
        None,
    )
    .unwrap();
    let at = w.tick;
    a::commit(
        w.rules.aaw.as_mut().unwrap(),
        a::Permit {
            key: "scenario_assumption: actual-source nuclear abort consumer terminal".into(),
            channel: 0,
            shooter,
            target,
            weapon: "us_essm_i".into(),
            tick: at,
            band: a::Band::Long1,
            budget,
            initial_course: 0.,
            initial_range: 0.01,
            attack,
            missiles: 1,
        },
        &[carrier],
    )
    .unwrap();
    let packet = w
        .rules
        .aaw
        .as_ref()
        .unwrap()
        .shots
        .get(&carrier)
        .expect("actual returned carrier must be registered in its real source packet");
    assert_eq!(packet.permit.shooter, shooter);
    assert_eq!(packet.permit.target, target);
    assert!(packet.members.contains(&carrier));
    let since = w.event_seq;
    while w.tick<at+120 && !w.events.iter().any(|e|e.seq>=since&&matches!(&e.kind,EventKind::RulesEvent{rule,detail,..} if rule=="14.1.8.detonation"&&detail.contains("\"failed\":false"))) {
        assert!(!w.over);harpoon_core::sim::tick(&mut w,&[]);
    }
    assert!(
        w.rules
            .aaw
            .as_ref()
            .unwrap()
            .outcomes
            .values()
            .any(|o| o.consumed_projectiles.contains(&carrier) && o.hit),
        "real source terminal must succeed before a real burst can establish mission abort"
    );
    assert!(w.events.iter().any(|e|e.seq>=since&&matches!(&e.kind,EventKind::RulesEvent{rule,detail,..} if rule=="14.1.8.detonation"&&detail.contains("\"failed\":false"))),"real non-dud source burst is a fixture precondition, never an inserted abort flag");
    let aircraft = w.entity(air).unwrap();
    assert!(aircraft.alive);
    assert_eq!(aircraft.ext.get("nuclear.mission_abort"), Some(&1.));
    assert_eq!(
        aircraft.hp, aircraft.hp_max,
        "source2psi abort does not authorize an invented aircraft DP formula"
    );
    assert!(w.entity(id(&w, "TARGET")).is_some_and(|e| e.alive));
    if comms::received_located_report(&w, air, id(&w, "TARGET"))
        .unwrap()
        .is_none()
    {
        let observer = id(&w, "OBSERVER");
        assert!(w.entity(observer).is_some_and(|e|e.alive&&e.systems.sonar>0.),"original independent source observer must really survive before any new report can qualify");
        let until = w.tick + 360;
        let mut sent = false;
        while w.tick < until {
            if let Some(cid) = w.sides[0]
                .contacts
                .values()
                .find(|c| c.about == id(&w, "TARGET") && !c.stale && !c.bearing_only)
                .map(|c| c.id)
            {
                let report = cmd(
                    observer,
                    "communications",
                    json!({"type":"send_warning","contact":cid,"sensor":SONAR,"fit":"voice","receivers":[air]}),
                );
                // Send-warning deliberately permits an immutable historical
                // snapshot. Its admission alone therefore cannot prove the
                // stronger live source qualification used by aerial ASW.
                let fresh_native = comms::native_report_view(&w, observer, cid, SONAR)
                    .ok()
                    .is_some_and(|r| {
                        r["observed_at"].as_i64() == Some(w.tick)
                            && !r["measured_position"].is_null()
                    });
                if fresh_native && report.validate_source(&w, Side::Blue).is_ok() {
                    harpoon_core::sim::tick(&mut w, &[(Side::Blue, report)]);
                    if comms::received_located_report(&w, air, id(&w, "TARGET"))
                        .unwrap()
                        .is_some()
                    {
                        sent = true;
                        break;
                    }
                    continue;
                }
            }
            assert!(!w.over);
            harpoon_core::sim::tick(&mut w, &[]);
        }
        assert!(sent,"original alive observer must acquire a real sendable native location, never a filled report");
    }
    assert!(comms::received_located_report(&w,air,id(&w,"TARGET")).unwrap().is_some(),"actual newly delivered or still-live original location must prove ASW localization after the nuclear event");
    assert!(w.entity(air).is_some_and(|e|e.alive&&e.altitude_m>0.&&e.ext.get("nuclear.mission_abort")==Some(&1.)),"original mission-aborted aircraft must remain actually airborne/live through the real location refresh");
    assert!(ammo(&w, air) > 0);
    assert!(
        !w.over,
        "the runtime submit case must still be an active simulation after the real burst"
    );
    assert!(w.rules.aviation.as_ref().unwrap().flights[&air].fuel_nm > 0.);
    // The actual already-damaged aircraft, not a newly cloned airframe.
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,unit,..} if rule=="14.1.8.mission_abort"&&*unit==Some(air))));
    n::step(&mut w); // already resolved burst is idempotent, not a new event.
    w
}
#[test]
fn na01_real_two_psi_mission_abort_blocks_readonly_direct_aerial_asw_admission() {
    let w = real_abort_fixture();
    let air = id(&w, "AIR");
    let stock = ammo(&w, air);
    let c = proposed_drop(&w);
    assert!(c.validate_source(&w,Side::Blue).is_err(),"14.1.8 actual mission-abort cannot be bypassed by the direct Asw transaction's chapter9 release gate");
    assert_eq!(ammo(&w, air), stock);
}
#[test]
fn na02_real_two_psi_mission_abort_blocks_actual_asw_submit_without_spending_or_creating_a_pattern()
{
    let mut w = real_abort_fixture();
    let air = id(&w, "AIR");
    let stock = ammo(&w, air);
    let c = proposed_drop(&w);
    let since = w.event_seq;
    // Submit through the normal runtime even when the caller skips preflight.
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, c)]);
    assert_eq!(ammo(&w,air),stock,"actual mission-aborted aircraft cannot spend a store after normal direct command submission");
    assert!(!w.events.iter().any(|e| e.seq >= since
        && matches!(&e.kind,EventKind::WeaponsRelease{shooter,..} if *shooter==air)));
    assert!(!w.events.iter().any(|e|e.seq>=since&&matches!(&e.kind,EventKind::RulesEvent{rule,unit,..} if rule=="8.5.4.4.pattern_committed"&&*unit==Some(air))));
}

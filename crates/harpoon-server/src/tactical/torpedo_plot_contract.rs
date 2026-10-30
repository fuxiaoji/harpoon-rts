//! Policy adapters over explicit own Plans and carried source weapon profiles.
use super::*;
use harpoon_core::{observe, scenario_from_json, Vec2};
use serde_json::json;

pub(super) fn public_weapon_profile(weapon: &str) -> serde_json::Value {
    let source = harpoon_core::rules::torpedo2026::tables()["weapons"][weapon].clone();
    json!({"guidance":source["guidance"],"generation":source["generation"],
        "settings":source["settings"],"max_depth_m":source["max_depth_m"],
        "surface":source["surface"],"submarine":source["submarine"],"source":source["source"]})
}

fn state() -> (TacticalStaff, Observation, EntityId) {
    let mut db =
        Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
    // Counterfactual generic range deliberately differs from the selected
    // source setting. It is never evidence for the actual Mk48 performance.
    db.weapons.get_mut("us_mk48").unwrap().range_nm = 40.;
    let sc=scenario_from_json(r#"{"id":"torpedo-policy","name":"Author controller fixture","duration_s":1000,"seed":1,"context":{"rules_profile":"rts-legacy-v1"},"weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"ffg","name":"OWN","pos":[0,0],"fire_on":false},{"side":"Blue","class":"us_ford","name":"CV","pos":[0,-10],"fire_on":false},{"side":"Red","class":"ffg","name":"SECRET","pos":[999,999],"fire_on":false}]}"#).unwrap();
    let world = harpoon_core::load_scenario(sc, db.clone()).unwrap();
    let mut obs = observe(&world, Side::Blue, 0, 0);
    obs.rules_profile = harpoon_core::rules::damage_runtime::RuleProfile::Rts2026Full;
    for u in &mut obs.units {
        if let Some(deck) = u.deck.as_mut() {
            deck.ready.clear();
            deck.cycling.clear();
        }
        u.weapons.clear();
    }
    let unit = obs.units[0].id;
    let mut mount = observe(&world, Side::Blue, 0, 0).units[0].weapons[0].clone();
    mount.weapon = "us_mk48".into();
    mount.ammo = 4;
    mount.cooldown_s = 0.;
    mount.operational = true;
    let u = &mut obs.units[0];
    u.kind = PlatformKind::Sub;
    u.class = "cn_093".into();
    u.name = "scenario_assumption: SUB adapter".into();
    u.depth_m = 30.;
    u.deck = None;
    u.weapons = vec![mount];
    u.torpedoes = Some(
        json!({"verified":true,"tubes":[{"index":0,"loaded":"us_mk48","servicing":false},{"index":1,"loaded":"us_mk48","servicing":false}],"reserves":{},"reloads":[],"releases":[],"plans":[],"weapon_profiles":{"us_mk48":public_weapon_profile("us_mk48")}}),
    );
    for (id, distance) in [(71, 5.), (72, 20.)] {
        obs.contacts.push(serde_json::from_value(json!({"id":id,"category":"Ship","class_name":null,"estimate":{"x":0,"y":distance},"est_course_deg":0,"est_speed_kts":0,"quality":1,"bearing_only":false,"last_seen_s":0,"stale":false,"sensors":[],"own_in_flight":0})).unwrap());
    }
    let mut staff = TacticalStaff::with_db(Side::Blue, db);
    staff.params.cap_min_size = 0;
    (staff, obs, unit)
}
fn plan(obs: &mut Observation, unit: EntityId, contact: u32, weapon: &str) {
    obs.units
        .iter_mut()
        .find(|u| u.id == unit)
        .unwrap()
        .torpedoes
        .as_mut()
        .unwrap()["plans"] = json!([{"actor":unit,"side":"Blue","contact":contact,"weapon":weapon,"plotted_at":0,"speed_index":1,"depth_m":30,"enable_run_nm":8,"bearing_deg":0,"spread_deg":[0],"cut_wire":false}]);
}
fn shots(commands: &[Command], unit: EntityId) -> Vec<(ContactId, u8)> {
    commands
        .iter()
        .filter_map(|c| {
            if let Command::Fire {
                unit: id,
                contact,
                rounds,
                ..
            } = c
            {
                (*id == unit).then_some((*contact, *rounds))
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn source_torpedo_fsm_holds_without_an_own_plan_and_keeps_legacy_release() {
    let (mut staff, mut obs, unit) = state();
    assert!(shots(&staff.decide(&obs), unit).is_empty());
    obs.rules_profile = harpoon_core::rules::damage_runtime::RuleProfile::Legacy;
    assert!(
        !shots(&staff.decide(&obs), unit).is_empty(),
        "legacy release is independent of new source plans"
    );
}
#[test]
fn source_torpedo_fsm_uses_selected_plan_and_does_not_break_its_own_wire_intent() {
    let (mut staff, mut obs, unit) = state();
    plan(&mut obs, unit, 72, "us_mk48");
    let commands = staff.decide(&obs);
    assert_eq!(
        shots(&commands, unit),
        vec![(ContactId(72), 1)],
        "a single declared spread entry is not permission for an undeclared double shot"
    );
    assert!(!commands
        .iter()
        .any(|c| matches!(c,Command::SetSpeed{unit:id,speed_kts}if *id==unit&&*speed_kts>10.)));
    assert!(!commands.iter().any(|c|matches!(c,Command::SetCourse{unit:id,course_deg}if *id==unit&&harpoon_core::geom::angle_diff(*course_deg,obs.units[0].course_deg).abs()>45.)));
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::Torpedo { .. })),
        "helper consumes an existing plan instead of inventing Plot/WireOrder/CutWire"
    );
}
#[test]
fn source_torpedo_fsm_consumes_the_selected_source_setting_instead_of_generic_range() {
    let (mut staff, mut obs, unit) = state();
    plan(&mut obs, unit, 72, "us_mk48");
    obs.units[0].torpedoes.as_mut().unwrap()["plans"][0]["speed_index"] = json!(0);
    assert!(
        shots(&staff.decide(&obs), unit).is_empty(),
        "the 15nm setting cannot authorize a 20nm report just because generic range is40"
    );
}
#[test]
fn source_torpedo_fsm_rejects_missing_profiles_invalid_plans_and_unknown_release_status() {
    for invalid in [
        "profile",
        "setting",
        "actor",
        "weapon",
        "stale",
        "bearing",
        "nonfinite",
        "unknown_allocation",
        "disabled",
        "servicing",
    ] {
        let (mut staff, mut obs, unit) = state();
        plan(&mut obs, unit, 72, "us_mk48");
        match invalid {
            "profile" => obs.units[0].torpedoes.as_mut().unwrap()["weapon_profiles"] = json!({}),
            "setting" => {
                obs.units[0].torpedoes.as_mut().unwrap()["plans"][0]["speed_index"] = json!(999)
            }
            "actor" => obs.units[0].torpedoes.as_mut().unwrap()["plans"][0]["actor"] = json!(999),
            "weapon" => {
                obs.units[0].torpedoes.as_mut().unwrap()["plans"][0]["weapon"] = json!("unmounted")
            }
            "stale" => obs.contacts[1].stale = true,
            "bearing" => obs.contacts[1].bearing_only = true,
            "nonfinite" => obs.contacts[1].estimate.x = f64::NAN,
            "unknown_allocation" => {
                obs.contacts[1].own_in_flight = None;
                obs.contacts[1].own_torpedo_releases =
                    Some(json!({"status":"unknown","physical_in_flight":null}));
            }
            "disabled" => obs.units[0].weapons[0].operational = false,
            "servicing" => {
                for t in obs.units[0].torpedoes.as_mut().unwrap()["tubes"]
                    .as_array_mut()
                    .unwrap()
                {
                    t["servicing"] = json!(true);
                }
            }
            _ => unreachable!(),
        }
        assert!(
            shots(&staff.decide(&obs), unit).is_empty(),
            "invalid own source plan must hold: {invalid}"
        );
    }
}
#[test]
fn source_torpedo_fsm_respects_human_takeover_and_only_resumes_the_existing_own_plan() {
    let (mut staff, mut obs, unit) = state();
    plan(&mut obs, unit, 72, "us_mk48");
    obs.units[0].manual_control = true;
    assert!(staff.decide(&obs).iter().all(|c| c.unit_id() != unit));
    obs.units[0].manual_control = false;
    assert_eq!(shots(&staff.decide(&obs), unit), vec![(ContactId(72), 1)]);
}
#[test]
fn source_torpedo_air_helper_needs_a_plan_in_addition_to_actual_release_posture() {
    let (mut staff, mut obs, unit) = state();
    for c in &mut obs.contacts {
        c.category = Some(PlatformKind::Sub);
        c.estimate.y = if c.id == ContactId(71) { 0.5 } else { 1. };
    }
    let u = &mut obs.units[0];
    u.kind = PlatformKind::Aircraft;
    u.class = "us_mh60r_mk54x4".into();
    u.name = "MH-60R source adapter".into();
    u.mission = Some("asw".into());
    u.altitude_m = 1000.;
    u.speed_kts = 0.;
    u.endurance_s = Some(10000.);
    u.weapons[0].weapon = "us_mk54".into();
    u.aviation = Some(
        json!({"profile":"hv2026-rts-v4","aircombat":{"aerial_torpedo_release":{"ready_at":0,"remaining_s":0,"max_altitude_m":2000,"max_speed_kts":70,"rotary_allow_hover":true}}}),
    );
    u.torpedoes = Some(
        json!({"verified":false,"tubes":null,"reloads":null,"reserves":null,"releases":[],"plans":[],"weapon_profiles":{"us_mk54":public_weapon_profile("us_mk54")}}),
    );
    assert!(
        shots(&staff.decide(&obs), unit).is_empty(),
        "actual aircraft readiness does not manufacture a torpedo prelaunch plan"
    );
    plan(&mut obs, unit, 72, "us_mk54");
    obs.units[0].torpedoes.as_mut().unwrap()["plans"][0]["enable_run_nm"] = json!(0);
    assert_eq!(shots(&staff.decide(&obs), unit), vec![(ContactId(72), 1)]);
    obs.units[0].aviation.as_mut().unwrap()["aircombat"]["aerial_torpedo_release"]["ready_at"] =
        json!(1000);
    assert!(
        shots(&staff.decide(&obs), unit).is_empty(),
        "an existing torpedo plan does not skip the real aircraft release clock"
    );
}

#[test]
fn source_torpedo_fsm_consumes_real_core_observation_public_weapon_profile() {
    use harpoon_core::rules::torpedo2026 as torp;
    let db = Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
    let sc = serde_json::from_value(json!({
        "id":"torpedo-real-public-projection","name":"Public projection adapter acceptance",
        "duration_s":1000,"seed":20261024,
        "context":{"rules_profile":"hv2026-rts-v4","torpedo_2026":{
            "source":"scenario_assumption: controlled prelaunch mechanism fixture, no real hull fit claim",
            "environment":{"source":"scenario_assumption: no layer","layer_m":null,"reverberant":false},
            "platforms":{"us_virginia":{"source":"scenario_assumption: two loaded mechanism tubes",
                "wire_capacity":2,"reload_parallel":2,"tubes":[{"bearing_deg":0,"loaded":"us_mk48"},
                {"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{"us_mk48":2}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_virginia","name":"OWN","pos":[0,0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"REFEREE","pos":[0,0.5],"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    let mut world = harpoon_core::load_scenario(sc, db.clone()).unwrap();
    let actor = world
        .entities
        .values()
        .find(|e| e.side == Side::Blue)
        .unwrap()
        .id;
    let target = world
        .entities
        .values()
        .find(|e| e.side == Side::Red)
        .unwrap()
        .id;
    // An explicit controlled own report tests the projection adapter. It is
    // not a native sonar-detection or valid firing-solution acceptance claim.
    world.sides[0].contacts.insert(
        ContactId(99),
        harpoon_core::world::Contact {
            id: ContactId(99),
            about: target,
            category: PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 0.5),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: 0,
            stale: false,
            sensors: vec![],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    world
        .rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert(format!("0:{}", target.0), PlatformKind::Sub);
    torp::apply(
        &mut world,
        actor,
        &torp::Action::Plot {
            contact: ContactId(99),
            weapon: "us_mk48".into(),
            speed_index: 1,
            depth_m: 50.,
            enable_run_nm: 0.,
            bearing_deg: Some(0.),
            spread_deg: vec![0.],
            cut_wire: false,
        },
    )
    .unwrap();
    let obs = observe(&world, Side::Blue, 0, 0);
    let own = obs.units.iter().find(|u| u.id == actor).unwrap();
    let profile = &own.torpedoes.as_ref().unwrap()["weapon_profiles"]["us_mk48"];
    assert_eq!(profile.as_object().unwrap().len(), 7);
    assert!(profile.get("shallow_water_capable").is_none());
    assert_eq!(own.torpedoes.as_ref().unwrap()["plans"][0]["contact"], 99);
    let mut staff = TacticalStaff::with_db(Side::Blue, db);
    staff.params.cap_min_size = 0;
    assert_eq!(shots(&staff.decide(&obs), actor), vec![(ContactId(99), 1)],
        "the helper must consume the real seven-field public DTO instead of requiring a private full Weapon");
}

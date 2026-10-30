//! Own-observation policy fixtures, independent of the source executor's dice.
use super::*;
use harpoon_core::{observe, scenario_from_json};
use serde_json::json;

fn state() -> (TacticalStaff, Observation, EntityId) {
    let mut db = Database::built_in();
    let mut weapon = db
        .weapons
        .values()
        .find(|w| w.kind == WeaponKind::Ascm)
        .unwrap()
        .clone();
    weapon.id = "us_rgm84d_ic".into();
    weapon.range_nm = 80.; // Explicit adapter fixture; real source data are separately tested in core.
    db.weapons.insert(weapon.id.clone(), weapon);
    let sc=scenario_from_json(r#"{"id":"ascm-policy","name":"Author adapter fixture","duration_s":1000,"seed":1,"weather":{"visibility_nm":0,"jam":[0,0]},"units":[{"side":"Blue","class":"ffg","name":"OWN","pos":[0,0],"fire_on":false},{"side":"Red","class":"ffg","name":"SECRET","pos":[99,88],"fire_on":false}]}"#).unwrap();
    let world = harpoon_core::load_scenario(sc, db.clone()).unwrap();
    let mut obs = observe(&world, Side::Blue, 0, 0);
    obs.rules_profile = harpoon_core::rules::damage_runtime::RuleProfile::Rts2026Full;
    let unit = obs.units[0].id;
    obs.units[0].deck = None;
    obs.units[0].weapons.truncate(1);
    obs.units[0].weapons[0].weapon = "us_rgm84d_ic".into();
    obs.units[0].weapons[0].ammo = 4;
    obs.units[0].weapons[0].cooldown_s = 0.;
    for (id, y) in [(71, 10.), (72, 40.)] {
        obs.contacts.push(serde_json::from_value(json!({"id":id,"category":"Ship","class_name":null,"estimate":{"x":0,"y":y},"est_course_deg":0,"est_speed_kts":0,"quality":1,"bearing_only":false,"last_seen_s":0,"stale":false,"sensors":[],"own_in_flight":0})).unwrap());
    }
    (TacticalStaff::with_db(Side::Blue, db), obs, unit)
}
fn orders(obs: &mut Observation, contact: u64) {
    let unit = obs.units[0].id;
    obs.units[0].antiship = Some(
        json!({"profile":"hv2026-rts-v4","execution":"source_executor","plots":[{"actor":unit,"side":"Blue","plotted_at":0,"contact":contact,"weapon":"us_rgm84d_ic","course_deg":0,"speed_kts":561,"enable_run_nm":40,"waypoints":[],"popup":false}]}),
    );
}
fn fires(staff: &mut TacticalStaff, obs: &Observation) -> Vec<Command> {
    let mut commands = vec![];
    for tick in [0, 30, 60, 90, 120, 150, 180] {
        let mut current = obs.clone();
        current.tick = tick;
        commands.extend(staff.decide(&current));
    }
    commands
        .into_iter()
        .filter(|c| matches!(c, Command::Fire { .. } | Command::Antiship { .. }))
        .collect()
}

#[test]
fn source_antiship_fsm_holds_without_a_plot_and_never_invents_enable_orders() {
    let (mut staff, obs, _) = state();
    assert!(
        fires(&mut staff, &obs).is_empty(),
        "v4 source ASCM cannot inherit a default enable point or generic Fire decision"
    );
}
#[test]
fn source_antiship_fsm_consumes_the_plotted_contact_instead_of_the_nearest_ship() {
    let (mut staff, mut obs, unit) = state();
    orders(&mut obs, 72);
    let commands = fires(&mut staff, &obs);
    assert!(commands.iter().any(|c|matches!(c,Command::Fire{unit:u,contact,weapon,..} if *u==unit&&*contact==ContactId(72)&&weapon=="us_rgm84d_ic")),"existing own plot should be actionable under actual engine gates: {commands:?}");
    assert!(!commands
        .iter()
        .any(|c| matches!(c,Command::Fire{contact,..}if *contact==ContactId(71))));
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::Antiship { .. })),
        "the helper must not rewrite player plot declarations"
    );
}
#[test]
fn source_antiship_fsm_requires_current_located_contact_and_known_allocation() {
    for invalid in [
        "missing",
        "stale",
        "bearing",
        "nonfinite",
        "unknown_allocation",
        "wrong_actor",
    ] {
        let (mut staff, mut obs, _) = state();
        orders(&mut obs, 72);
        match invalid {
            "missing" => obs.contacts.retain(|c| c.id != ContactId(72)),
            "stale" => obs.contacts[1].stale = true,
            "bearing" => obs.contacts[1].bearing_only = true,
            "nonfinite" => obs.contacts[1].estimate.x = f64::NAN,
            "unknown_allocation" => obs.contacts[1].own_in_flight = None,
            "wrong_actor" => {
                obs.units[0].antiship.as_mut().unwrap()["plots"][0]["actor"] = json!(99999)
            }
            _ => unreachable!(),
        }
        assert!(
            fires(&mut staff, &obs).is_empty(),
            "invalid own plot/contact must hold ({invalid})"
        );
    }
}
#[test]
fn source_antiship_fsm_preserves_manual_takeover_and_resumes_only_after_explicit_release() {
    let (mut staff, mut obs, _) = state();
    orders(&mut obs, 72);
    obs.units[0].manual_control = true;
    assert!(staff.decide(&obs).is_empty());
    obs.units[0].manual_control = false;
    assert!(fires(&mut staff, &obs)
        .iter()
        .any(|c| matches!(c,Command::Fire{contact,..}if *contact==ContactId(72))));
}
#[test]
fn source_antiship_fsm_keeps_legacy_surface_release_without_source_plot() {
    let (mut staff, mut obs, _) = state();
    obs.rules_profile = harpoon_core::rules::damage_runtime::RuleProfile::Legacy;
    assert!(fires(&mut staff, &obs)
        .iter()
        .any(|c| matches!(c,Command::Fire{contact,..}if *contact==ContactId(71))));
}

#[test]
fn source_antiship_fsm_uses_one_round_without_guessing_an_unobserved_launcher_limit() {
    // The actor has eight rounds, while an author may permit four per turn.
    // That referee-only rate is deliberately absent from the own observation:
    // this case checks an explicit conservative AI policy, not source rate data.
    let (mut staff, mut obs, _) = state();
    orders(&mut obs, 72);
    obs.units[0].weapons[0].ammo = 8;
    let commands = fires(&mut staff, &obs);
    let rounds: Vec<_> = commands
        .iter()
        .filter_map(|c| {
            if let Command::Fire { rounds, .. } = c {
                Some(*rounds)
            } else {
                None
            }
        })
        .collect();
    assert!(!rounds.is_empty());
    assert!(rounds.iter().all(|n|*n==1),"source helper must submit one explicit conservative round instead of treating eight stocked rounds as source turn capacity: {rounds:?}");
    let (mut legacy, mut old, _) = state();
    old.rules_profile = harpoon_core::rules::damage_runtime::RuleProfile::Legacy;
    old.units[0].weapons[0].ammo = 8;
    assert!(
        fires(&mut legacy, &old)
            .iter()
            .any(|c| matches!(c,Command::Fire{rounds,..}if *rounds>1)),
        "historical salvo policy remains separate"
    );
}

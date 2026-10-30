//! Unregistered candidates: no new Rust production type is required to compile.
//! English2026 PDF116/11-1 and PDF118/11-3: disabled ship berth prevents
//! alongside resupply. Stock, capacity, layout and timing below are explicitly
//! author-declared experimental inputs, never universal Harpoon/Annex values.
use harpoon_core::rules::ground2026 as ground;
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::{json, Value};

fn scenario_value() -> Value {
    json!({
        "id":"berth-service-source-lab","name":"scenario_assumption: independent experimental depot and alongside services",
        "seed":26,"duration_s":600,"weather":{"visibility_nm":40,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","ground_2026":{
            "source":"scenario_assumption: isolated logistics experiment; no real naval-base fit",
            "facilities":{
                "BERTH":{"template":"ship_berth","custom":null,"source":"English2026 PDF118 Ship Berth; laboratory placement","parent":"DEPOT"}},
            "berth_depots":{"DEPOT":{"source":"scenario_assumption: new independent initial inventory, not copied from any Entity ammunition","stocks":{"cn_yj18":3}}},
            "berth_services":{
                "lab":{"source":"scenario_assumption: explicit experimental assignment/geometry/capacity/time",
                    "depot":"DEPOT","berth":"BERTH","recipient":"SHIP","mount_index":1,"weapon":"cn_yj18",
                    "capacity":4,"radius_nm":0.01,"max_speed_kts":0.0,"seconds_per_round":3},
                "lab2":{"source":"scenario_assumption: second recipient shares the same original three rounds",
                    "depot":"DEPOT","berth":"BERTH","recipient":"SHIP2","mount_index":1,"weapon":"cn_yj18",
                    "capacity":4,"radius_nm":0.01,"max_speed_kts":0.0,"seconds_per_round":3}}}},
        "units":[
            {"side":"Blue","class":"exercise_airbase","name":"DEPOT","pos":[0,0],"fire_on":false},
            {"side":"Blue","class":"exercise_airbase","name":"BERTH","pos":[0,0],"fire_on":false},
            {"side":"Blue","class":"cn_052d","name":"SHIP","pos":[0,0],"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":"cn_052d","name":"SHIP2","pos":[0,0],"speed_kts":0,"fire_on":false}]
    })
}
fn database() -> Database {
    let mut value: Value =
        serde_json::from_str(include_str!("../../../database/consolidated.json")).unwrap();
    // Explicit experimental INITIAL ammunition only. Original files, weapon
    // performance and physical mount count are untouched. This is no claim
    // about a real 052D's starting stock or logistics installation.
    value["platforms"]["cn_052d"]["weapons"][1]["ammo"] = json!(2);
    Database::from_json(&value.to_string()).unwrap().merged(
        &Database::from_json(include_str!(
            "../../../database/volumes/scenario_exercise_assets.json"
        ))
        .unwrap(),
    )
}
fn fixture() -> (World, EntityId, EntityId, EntityId, EntityId) {
    let world = harpoon_core::load_scenario(
        serde_json::from_value::<Scenario>(scenario_value()).unwrap(),
        database(),
    )
    .unwrap();
    let id = |name: &str| world.entities.values().find(|e| e.name == name).unwrap().id;
    let ids = (id("DEPOT"), id("BERTH"), id("SHIP"), id("SHIP2"));
    // Earliest expected baseline red is schema preservation. It must not be
    // misreported as an observed stock or clock consumer failure.
    let config = serde_json::to_value(world.context.ground_2026.as_ref().unwrap()).unwrap();
    assert_eq!(config["berth_depots"]["DEPOT"]["stocks"]["cn_yj18"],3,
        "explicit independent depot schema must survive before downstream service assertions are assessed");
    assert_eq!(config["berth_services"]["lab"]["recipient"], "SHIP");
    (world, ids.0, ids.1, ids.2, ids.3)
}
fn command(ship: EntityId, service: &str, rounds: u16) -> Command {
    serde_json::from_value(json!({"cmd":"ground","unit":ship,"action":{"type":"alongside_resupply","service":service,"rounds":rounds}})).unwrap()
}
fn cancel(ship: EntityId, service: &str) -> Command {
    serde_json::from_value(json!({"cmd":"ground","unit":ship,"action":{"type":"cancel_alongside_resupply","service":service}})).unwrap()
}
fn advance(world: &mut World, to: i64) {
    while world.tick < to {
        harpoon_core::sim::tick(world, &[]);
    }
}
fn order(world: &mut World, cmd: Command) {
    harpoon_core::sim::tick(world, &[(Side::Blue, cmd)]);
}
fn ammo(world: &World, ship: EntityId) -> u16 {
    world.entity(ship).unwrap().weapons[1].ammo
}
fn service(world: &World, ship: EntityId, name: &str) -> Value {
    let result = ground::own_view(world, ship).unwrap()["berth_resupply"]["services"][name].clone();
    assert!(result.is_object(),"recipient's own view must expose its explicit service, even though the ship is not a Facility");
    result
}
fn no_transfer(world: &World, ship: EntityId) {
    assert_eq!(ammo(world, ship), 2);
    let s = service(world, ship, "lab");
    assert_eq!(s["stock"], 3);
    assert_eq!(s["reserved_stock"], 0);
    assert!(s["job"].is_null());
}
fn disable(world: &mut World, berth: EntityId) {
    // Already-adjudicated penetrated source damage at the native consumer
    // boundary, not a claim of a naturally fired/penetrating weapon attack.
    ground::apply_damage(
        world,
        berth,
        200.,
        0,
        "scenario_assumption: adjudicated exact half-DP berth casualty",
    )
    .unwrap();
    assert!(!ground::operational(world, berth));
}
fn has_rule(world: &World, rule: &str) -> bool {
    world.events.iter().any(
        |e| matches!(&e.kind,EventKind::RulesEvent{domain,rule:r,..}if domain=="ground"&&r==rule),
    )
}

#[test]
fn b01_explicit_shared_depot_and_service_assignments_survive_configuration_roundtrip() {
    let (world, _, _, ship, ship2) = fixture();
    let a = service(&world, ship, "lab");
    let b = service(&world, ship2, "lab2");
    assert_eq!(a["stock"], 3);
    assert_eq!(b["stock"], 3);
    assert_eq!(a["capacity"], 4);
    assert_eq!(a["reserved_stock"], 0);
    assert!(
        ground::own_view(&world, ship).unwrap()["berth_resupply"]["services"]["lab2"].is_null(),
        "another recipient's service is not silently assigned to this ship"
    );
}
#[test]
fn b02_real_ground_command_reserves_then_atomically_debits_and_credits_once() {
    let (mut world, _, _, ship, _) = fixture();
    let cmd = command(ship, "lab", 2);
    let before = serde_json::to_value(&world).unwrap();
    assert!(cmd.validate_source(&world, Side::Blue).is_ok());
    assert_eq!(
        before,
        serde_json::to_value(&world).unwrap(),
        "source preflight must be read only"
    );
    order(&mut world, cmd);
    let started = world.tick;
    let queued = service(&world, ship, "lab");
    assert_eq!(ammo(&world, ship), 2);
    assert_eq!(queued["stock"], 3);
    assert_eq!(queued["reserved_stock"], 2);
    assert_eq!(queued["reserved_capacity"], 2);
    assert_eq!(queued["job"]["due"], started + 6);
    advance(&mut world, started + 5);
    assert_eq!(ammo(&world, ship), 2);
    advance(&mut world, started + 6);
    assert_eq!(ammo(&world, ship), 4);
    let done = service(&world, ship, "lab");
    assert_eq!(done["stock"], 1);
    assert_eq!(done["reserved_stock"], 0);
    assert!(done["job"].is_null());
    advance(&mut world, started + 25);
    assert_eq!(ammo(&world, ship), 4);
    assert_eq!(service(&world, ship, "lab")["stock"], 1);
    assert!(has_rule(&world, "11.berth.transfer_completed"));
}
#[test]
fn b03_half_dp_source_berth_loss_rejects_actual_new_service_without_free_credit() {
    let (mut world, _, berth, ship, _) = fixture();
    disable(&mut world, berth);
    let cmd = command(ship, "lab", 2);
    let before = serde_json::to_value(&world).unwrap();
    assert!(cmd.validate_source(&world, Side::Blue).is_err());
    assert_eq!(before, serde_json::to_value(&world).unwrap());
    order(&mut world, cmd);
    advance(&mut world, 20);
    no_transfer(&world, ship);
    assert!(world.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{domain,rule,unit:Some(u),detail,..}if domain=="command"&&rule=="rejected"&&*u==ship&&detail.starts_with("ground:"))));
}
#[test]
fn b04_source_berth_loss_cancels_queued_work_and_releases_both_reservations() {
    let (mut world, _, berth, ship, _) = fixture();
    order(&mut world, command(ship, "lab", 2));
    assert_eq!(service(&world, ship, "lab")["reserved_stock"], 2);
    advance(&mut world, 3);
    disable(&mut world, berth);
    advance(&mut world, 25);
    no_transfer(&world, ship);
    assert!(has_rule(&world, "11.berth.transfer_cancelled"));
}
#[test]
fn b05_two_recipients_cannot_oversubscribe_one_physical_depot_stock() {
    let (mut world, _, _, ship, ship2) = fixture();
    harpoon_core::sim::tick(
        &mut world,
        &[
            (Side::Blue, command(ship, "lab", 2)),
            (Side::Blue, command(ship2, "lab2", 2)),
        ],
    );
    assert_eq!(service(&world, ship, "lab")["reserved_stock"], 2);
    assert!(service(&world, ship2, "lab2")["job"].is_null());
    advance(&mut world, 15);
    assert_eq!(ammo(&world, ship), 4);
    assert_eq!(ammo(&world, ship2), 2);
    assert_eq!(service(&world, ship, "lab")["stock"], 1);
    assert_eq!(service(&world, ship2, "lab2")["stock"], 1);
}
#[test]
fn b06_explicit_cancellation_releases_original_stock_without_duplicating_it() {
    let (mut world, _, _, ship, _) = fixture();
    order(&mut world, command(ship, "lab", 2));
    order(&mut world, cancel(ship, "lab"));
    advance(&mut world, 15);
    no_transfer(&world, ship);
    order(&mut world, command(ship, "lab", 2));
    advance(&mut world, 30);
    assert_eq!(ammo(&world, ship), 4);
    assert_eq!(service(&world, ship, "lab")["stock"], 1);
}
#[test]
fn b07_pending_transfer_full_world_restore_continues_identically() {
    let (mut original, _, _, ship, _) = fixture();
    order(&mut original, command(ship, "lab", 2));
    advance(&mut original, 3);
    let mut restored: World =
        serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&original).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    advance(&mut original, 15);
    advance(&mut restored, 15);
    assert_eq!(ammo(&original, ship), 4);
    assert_eq!(service(&original, ship, "lab")["stock"], 1);
    assert_eq!(
        serde_json::to_value(&original).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}
#[test]
fn b08_real_movement_cancels_service_before_original_due_without_banking_work() {
    let (mut world, _, _, ship, _) = fixture();
    order(&mut world, command(ship, "lab", 2));
    let move_cmd: Command =
        serde_json::from_value(json!({"cmd":"set_speed","unit":ship,"speed_kts":1})).unwrap();
    order(&mut world, move_cmd);
    advance(&mut world, 20);
    assert!(world.entity(ship).unwrap().speed_kts > 0.);
    no_transfer(&world, ship);
}
#[test]
fn b09_unknown_private_fields_and_wrong_side_are_rejected_without_writing_world() {
    let (world, _, _, ship, _) = fixture();
    let before = serde_json::to_value(&world).unwrap();
    let bad = serde_json::from_value::<Command>(
        json!({"cmd":"ground","unit":ship,"action":{"type":"alongside_resupply","service":"lab","rounds":2,"stock":999}}),
    );
    assert!(bad.is_err());
    assert_eq!(before, serde_json::to_value(&world).unwrap());
    assert!(command(ship, "lab", 2)
        .validate_source(&world, Side::Red)
        .is_err());
    assert_eq!(before, serde_json::to_value(&world).unwrap());
}
#[test]
fn b10_unsupported_torpedo_replenishment_is_not_misrepresented_as_aggregate_reload() {
    let _positive = fixture();
    let mut config = scenario_value();
    config["context"]["ground_2026"]["berth_depots"]["DEPOT"]["stocks"] = json!({"cn_yu7":3});
    for key in ["lab", "lab2"] {
        config["context"]["ground_2026"]["berth_services"][key]["weapon"] = json!("cn_yu7");
        config["context"]["ground_2026"]["berth_services"][key]["mount_index"] = json!(7);
        config["context"]["ground_2026"]["berth_services"][key]["capacity"] = json!(12);
    }
    assert!(
        harpoon_core::load_scenario(serde_json::from_value(config).unwrap(), database()).is_err()
    );
}

// Append-only addition to the registered ea69ac... candidate. It shares the
// same file's fixture/helpers; no original candidate or receipt is overwritten.
#[test]
fn b11_reversing_same_tick_competing_commands_preserves_inventory_and_service_winner() {
    let (world, _, _, ship, ship2) = fixture();
    let mut forward = world.clone();
    let mut reversed = world;
    harpoon_core::sim::tick(
        &mut forward,
        &[
            (Side::Blue, command(ship, "lab", 2)),
            (Side::Blue, command(ship2, "lab2", 2)),
        ],
    );
    harpoon_core::sim::tick(
        &mut reversed,
        &[
            (Side::Blue, command(ship2, "lab2", 2)),
            (Side::Blue, command(ship, "lab", 2)),
        ],
    );
    for (unit, name) in [(ship, "lab"), (ship2, "lab2")] {
        assert_eq!(
            service(&forward, unit, name),
            service(&reversed, unit, name),
            "same phase winner/reservations must not depend on client call ordering"
        );
    }
    assert!(
        !service(&forward, ship, "lab")["job"].is_null(),
        "stable actor/service ordering makes the same winner in both command orders"
    );
    assert!(service(&forward, ship2, "lab2")["job"].is_null());
    advance(&mut forward, 15);
    advance(&mut reversed, 15);
    assert_eq!((ammo(&forward, ship), ammo(&forward, ship2)), (4, 2));
    assert_eq!(
        (ammo(&forward, ship), ammo(&forward, ship2)),
        (ammo(&reversed, ship), ammo(&reversed, ship2))
    );
    assert_eq!(
        service(&forward, ship, "lab"),
        service(&reversed, ship, "lab")
    );
    // Command-receipt event order is the submitted command log. The assertion
    // deliberately compares authoritative service/stock results, not a log
    // that is expected to record different submitted order.
}
#[test]
fn b12_same_phase_cancel_wins_over_start_in_both_client_orders() {
    let (world, _, _, ship, _) = fixture();
    let mut forward = world.clone();
    let mut reversed = world;
    harpoon_core::sim::tick(
        &mut forward,
        &[
            (Side::Blue, command(ship, "lab", 2)),
            (Side::Blue, cancel(ship, "lab")),
        ],
    );
    harpoon_core::sim::tick(
        &mut reversed,
        &[
            (Side::Blue, cancel(ship, "lab")),
            (Side::Blue, command(ship, "lab", 2)),
        ],
    );
    advance(&mut forward, 20);
    advance(&mut reversed, 20);
    no_transfer(&forward, ship);
    no_transfer(&reversed, ship);
    assert_eq!(
        service(&forward, ship, "lab"),
        service(&reversed, ship, "lab")
    );
}

#[test]
fn b13_native_depot_loss_cancels_pending_transfer_without_creating_inventory() {
    let (mut world, depot, _, ship, _) = fixture();
    order(&mut world, command(ship, "lab", 2));
    // Explicit adjudicated loss at the native sink consumer boundary; no
    // weapon engagement, penetration or storage explosion is claimed.
    harpoon_core::damage::sink(&mut world, depot);
    advance(&mut world, 20);
    no_transfer(&world, ship);
    assert!(has_rule(&world, "11.berth.transfer_cancelled"));
}

#[test]
fn b14_changed_ownership_cannot_reassign_original_stock_or_expose_enemy_ledger() {
    let (mut world, depot, berth, ship, _) = fixture();
    order(&mut world, command(ship, "lab", 2));
    // There is no capture mechanic in this scope. These are explicitly
    // adjudicated ownership-boundary inputs, not a natural capture claim.
    for id in [depot, berth, ship] {
        world.entity_mut(id).unwrap().side = Side::Red;
    }
    advance(&mut world, 20);
    assert_eq!(ammo(&world, ship), 2);
    let full = serde_json::to_value(&world).unwrap();
    assert!(full["rules"]["ground"]["berth"]["jobs"]
        .as_object()
        .unwrap()
        .is_empty());
    assert!(ground::own_view(&world, ship).unwrap()["berth_resupply"].is_null());
    let before = serde_json::to_value(&world).unwrap();
    assert!(command(ship, "lab", 2)
        .validate_source(&world, Side::Red)
        .is_err());
    assert_eq!(before, serde_json::to_value(&world).unwrap());
}

#[test]
fn b15_unknown_time_and_moving_service_are_rejected_as_unsupported_author_configs() {
    let _positive = fixture();
    for (field, value) in [
        ("seconds_per_round", json!(0)),
        ("max_speed_kts", json!(1.0)),
    ] {
        let mut config = scenario_value();
        config["context"]["ground_2026"]["berth_services"]["lab"][field] = value;
        assert!(
            harpoon_core::load_scenario(serde_json::from_value(config).unwrap(), database())
                .is_err()
        );
    }
}

#[test]
fn b16_old_unconfigured_ground_keeps_new_state_and_projection_absent() {
    let mut config = scenario_value();
    let ground_config = config["context"]["ground_2026"].as_object_mut().unwrap();
    ground_config.remove("berth_depots");
    ground_config.remove("berth_services");
    let world =
        harpoon_core::load_scenario(serde_json::from_value(config).unwrap(), database()).unwrap();
    let ship = world
        .entities
        .values()
        .find(|e| e.name == "SHIP")
        .unwrap()
        .id;
    assert!(serde_json::to_value(&world).unwrap()["rules"]["ground"]
        .get("berth")
        .is_none());
    assert!(ground::own_view(&world, ship)
        .unwrap()
        .get("berth_resupply")
        .is_none());
    assert!(
        serde_json::to_value(world.context.ground_2026.as_ref().unwrap())
            .unwrap()
            .get("berth_services")
            .is_none()
    );
}

#[test]
fn b17_conflicting_same_phase_quantities_reject_group_in_both_orders() {
    let (world, _, _, ship, _) = fixture();
    let mut forward = world.clone();
    let mut reversed = world;
    harpoon_core::sim::tick(
        &mut forward,
        &[
            (Side::Blue, command(ship, "lab", 1)),
            (Side::Blue, command(ship, "lab", 2)),
        ],
    );
    harpoon_core::sim::tick(
        &mut reversed,
        &[
            (Side::Blue, command(ship, "lab", 2)),
            (Side::Blue, command(ship, "lab", 1)),
        ],
    );
    advance(&mut forward, 20);
    advance(&mut reversed, 20);
    no_transfer(&forward, ship);
    no_transfer(&reversed, ship);
}

#[test]
fn b18_identical_same_phase_requests_coalesce_into_one_actual_transfer() {
    let (mut world, _, _, ship, _) = fixture();
    harpoon_core::sim::tick(
        &mut world,
        &[
            (Side::Blue, command(ship, "lab", 2)),
            (Side::Blue, command(ship, "lab", 2)),
        ],
    );
    assert_eq!(service(&world, ship, "lab")["reserved_stock"], 2);
    advance(&mut world, 20);
    assert_eq!(ammo(&world, ship), 4);
    assert_eq!(service(&world, ship, "lab")["stock"], 1);
    assert_eq!(world.events.iter().filter(|e| matches!(&e.kind, EventKind::RulesEvent {domain,rule,..} if domain=="ground" && rule=="11.berth.transfer_completed")).count(), 1);
}

#[test]
fn b19_special_magazine_target_cannot_be_reused_as_independent_generic_depot() {
    let _positive = fixture();
    let mut config = scenario_value();
    config["context"]["ground_2026"]["facilities"]["DEPOT"] = json!({
        "template":"magazine_bunker","custom":null,
        "source":"English2026 PDF118 Magazine Bunker; special D10 gun-ammo-only service is outside this declared first domain"
    });
    assert!(
        harpoon_core::load_scenario(serde_json::from_value(config).unwrap(), database()).is_err(),
        "a special source Magazine Bunker must remain blocked; generic independent-stock services cannot silently apply full ordinary availability to it"
    );
}

#[test]
fn b20_changed_recipient_side_cannot_receive_original_owners_private_service_cancellation() {
    let (mut world, _, _, ship, _) = fixture();
    order(&mut world, command(ship, "lab", 2));
    assert_eq!(service(&world, ship, "lab")["reserved_stock"], 2);
    let since = world.events.back().map_or(0, |e| e.seq);
    // Explicit adjudicated ownership-boundary input; no natural capture is
    // implemented or claimed by this logistics experiment.
    world.entity_mut(ship).unwrap().side = Side::Red;
    harpoon_core::sim::tick(&mut world, &[]);
    let leaked: Vec<_> = world.events.iter().filter(|e| e.seq > since && e.vis.visible_to(Side::Red))
        .filter(|e| matches!(&e.kind, EventKind::RulesEvent {domain,detail,..}
            if domain == "ground" && serde_json::from_str::<Value>(detail).ok().is_some_and(|v| v["service"] == "lab" || v["rounds"] == 2)))
        .collect();
    assert!(
        leaked.is_empty(),
        "new ownership must not move the original Blue service/rounds audit to Red: {leaked:#?}"
    );
    assert!(world.events.iter().any(|e| e.seq > since
        && matches!(e.vis, harpoon_core::Visibility::Side(Side::Blue) | harpoon_core::Visibility::Referee)
        && matches!(&e.kind, EventKind::RulesEvent {domain,rule,detail,..}
            if domain == "ground" && rule == "11.berth.transfer_cancelled"
                && serde_json::from_str::<Value>(detail).ok().is_some_and(|v| v["service"] == "lab"))));
    assert_eq!(ammo(&world, ship), 2);
}

#[test]
fn b21_changed_side_cannot_cancel_original_private_service_through_readonly_preflight() {
    let (mut world, _, _, ship, _) = fixture();
    order(&mut world, command(ship, "lab", 2));
    assert_eq!(service(&world, ship, "lab")["reserved_stock"], 2);
    // Explicit adjudicated ownership boundary, before the next native phase
    // cancels the old job. This is not a natural capture mechanic.
    world.entity_mut(ship).unwrap().side = Side::Red;
    let before = serde_json::to_value(&world).unwrap();
    let denied = cancel(ship, "lab").validate_source(&world, Side::Red);
    assert!(denied.is_err(), "changed-side actor must not guess/cancel an original private Blue service before automatic invalidation");
    let unknown = cancel(ship, "unseen-service")
        .validate_source(&world, Side::Red)
        .unwrap_err();
    let foreign = cancel(ship, "lab2")
        .validate_source(&world, Side::Red)
        .unwrap_err();
    assert_eq!(
        denied.unwrap_err(),
        unknown,
        "old private service existence is not disclosed through a different denial"
    );
    assert_eq!(foreign, unknown);
    assert_eq!(
        before,
        serde_json::to_value(&world).unwrap(),
        "every rejected preflight remains read only"
    );
}

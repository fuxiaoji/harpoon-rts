//! Root registered the first five candidates and observed four binding-schema
//! reds plus the unbound-fit green. The implemented bound-work consumer and
//! four added boundary cases await the next root combined validation.
//! RepairSpec.maintenance_provider explicitly binds the service provider.
//! A shared parent alone never establishes a service dependency.
//! Source: English2026 PDF116/11-1, PDF117/11-2, PDF118/11-3.
//! The 600-second / one-team baseline is reused from ground_2026.rs's existing
//! scenario-assumption fixture. Factor0.5 is an explicit parent-approved author
//! parameter for this experiment, not a multiplier printed in the rulebook.
use harpoon_core::rules::ground2026 as ground;
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::json;

fn fixture(
    provider_bound: bool,
    reduced_rate: Option<f64>,
) -> (World, EntityId, EntityId, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json"))
        .unwrap()
        .merged(
            &Database::from_json(include_str!(
                "../../../database/volumes/scenario_exercise_assets.json"
            ))
            .unwrap(),
        );
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"facility-consumer-next-lab",
        "name":"scenario_assumption: one runway crew and its explicitly declared sole maintenance shop",
        "seed":19,"duration_s":20000,
        "weather":{"visibility_nm":40,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","ground_2026":{
            "source":"scenario_assumption: isolated service dependency; not a real base fit",
            "facilities":{
                "RUNWAY":{"template":"runway","custom":null,"parent":"BASE",
                    "source":"scenario_assumption: service dependency only exists when maintenance_provider explicitly names SHOP",
                    "runway":{"length_m":2000,"baseline":"Large","repair":{
                        "seconds_per_cut":600,"teams":1,
                        "maintenance_provider":if provider_bound {Some("SHOP")} else {None},
                        "source":"scenario_assumption: baseline reused from existing ground_2026.rs fixture, not a source universal rate"}}},
                "SHOP":{"template":"maintenance_large","custom":null,"parent":"BASE",
                    "source":"scenario_assumption: declared laboratory repair-rate input only; no real base fit or printed multiplier claim",
                    "repair_rate_factor":reduced_rate}},
            "complexes":{"BASE":{"kind":"Airbase","source":"scenario_assumption: isolated laboratory",
                "components":{"runways":["RUNWAY"],"buildings":["SHOP"]}}}}},
        "units":[
            {"side":"Blue","class":"exercise_airbase","name":"BASE","pos":[0,0],"fire_on":false},
            {"side":"Blue","class":"exercise_airbase","name":"RUNWAY","pos":[0,0],"fire_on":false},
            {"side":"Blue","class":"exercise_airbase","name":"SHOP","pos":[0,0],"fire_on":false}]
    })).unwrap();
    let mut world = harpoon_core::load_scenario(scenario, db).unwrap();
    let id = |name: &str| world.entities.values().find(|e| e.name == name).unwrap().id;
    let (base, runway, shop) = (id("BASE"), id("RUNWAY"), id("SHOP"));
    // Declared penetrated source damage is accepted by the production facility
    // damage entry. This is not claimed to be a naturally fired weapon attack.
    ground::apply_damage(
        &mut world,
        runway,
        10.,
        0,
        "scenario_assumption: adjudicated runway cut",
    )
    .unwrap();
    (world, base, runway, shop)
}

fn assert_binding(world: &World, runway: EntityId) {
    let value =
        serde_json::to_value(&world.rules.ground.as_ref().unwrap().facilities[&runway].config)
            .unwrap();
    assert_eq!(
        value["runway"]["repair"]["maintenance_provider"], "SHOP",
        "new explicit provider schema must roundtrip before a consumer result can be assessed; current RepairSpec may ignore unknown JSON keys"
    );
}

fn cuts(world: &World, runway: EntityId) -> u32 {
    world.rules.ground.as_ref().unwrap().facilities[&runway].cuts
}

fn disable_provider(world: &mut World, shop: EntityId) {
    let original = ground::source_target(world, shop)
        .unwrap()
        .damage_points
        .unwrap();
    ground::apply_damage(
        world,
        shop,
        original / 2.,
        0,
        "scenario_assumption: adjudicated half-DP shop loss",
    )
    .unwrap();
    assert!(!ground::operational(world, shop));
    assert!(!world.rules.ground.as_ref().unwrap().facilities[&shop].destroyed);
}

fn advance_to(world: &mut World, tick: i64) {
    while world.tick < tick {
        harpoon_core::sim::tick(world, &[]);
    }
}

fn order(world: &mut World, base: EntityId, runway: EntityId) {
    harpoon_core::sim::tick(
        world,
        &[(
            Side::Blue,
            Command::Ground {
                unit: base,
                action: ground::Action::RepairRunway { runway },
            },
        )],
    );
}

fn assert_new_repair_rejection(world: &World, base: EntityId, since_seq: u64, reason: &str) {
    let commands: Vec<_> = world
        .events
        .iter()
        .filter(|event| event.seq > since_seq)
        .filter(
            |event| matches!(&event.kind, EventKind::RulesEvent {domain,..} if domain == "command"),
        )
        .collect();
    assert!(
        !world.over
            && commands.iter().any(|event| matches!(&event.kind,
                EventKind::RulesEvent {domain,rule,unit:Some(actor),detail,..}
                if domain == "command" && rule == "rejected" && *actor == base
                    && detail.starts_with("ground:") && detail.contains(reason)
            )),
        "actual new Ground rejection missing: tick={}, over={}, expected reason={reason:?}, command_events={commands:#?}",
        world.tick,
        world.over,
    );
}

#[test]
fn unknown_declared_maintenance_reduction_does_not_queue_a_full_rate_runway_repair() {
    let (mut world, base, runway, shop) = fixture(true, None);
    assert_binding(&world, runway);
    // Positive control proves valid current schema, source facility state,
    // declared crew and the actual Ground command -> queued repair path.
    let mut undamaged = world.clone();
    order(&mut undamaged, base, runway);
    assert_eq!(
        undamaged.rules.ground.as_ref().unwrap().repair_jobs.len(),
        1
    );

    disable_provider(&mut world, shop);
    order(&mut world, base, runway);

    // After the schema exists, expected behavioral red: the old implementation
    // unconditionally schedules tick + seconds_per_cut, ignoring the provider.
    // This is only a fail-closed unknown-rate guard, not full repair execution.
    assert!(
        world.rules.ground.as_ref().unwrap().repair_jobs.is_empty(),
        "an explicitly affected repair cannot silently use its full baseline rate when the source-required reduction is unknown"
    );
    assert_eq!(
        world.rules.ground.as_ref().unwrap().facilities[&runway].cuts,
        1
    );
    assert!(world.events.iter().any(|event| matches!(&event.kind,
        EventKind::RulesEvent {domain,rule,unit:Some(actor),detail,..}
        if domain == "command" && rule == "rejected" && *actor == base
            && detail.starts_with("ground:")
    )));
}

#[test]
fn an_unbound_old_fit_keeps_its_declared_baseline_despite_nearby_shop_damage() {
    let (mut world, base, runway, shop) = fixture(false, None);
    disable_provider(&mut world, shop);
    order(&mut world, base, runway);
    let started = world.tick;
    assert_eq!(world.rules.ground.as_ref().unwrap().repair_jobs.len(), 1);
    advance_to(&mut world, started + 599);
    assert_eq!(cuts(&world, runway), 1);
    harpoon_core::sim::tick(&mut world, &[]);
    assert_eq!(cuts(&world, runway), 0);
}

#[test]
fn declared_half_rate_delays_actual_bound_repair_and_survives_world_restore() {
    let (mut world, base, runway, shop) = fixture(true, Some(0.5));
    assert_binding(&world, runway);
    disable_provider(&mut world, shop);
    order(&mut world, base, runway);
    let started = world.tick;
    advance_to(&mut world, started + 300);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    advance_to(&mut world, started + 600);
    advance_to(&mut restored, started + 600);
    assert_eq!(
        cuts(&world, runway),
        1,
        "half author-declared rate cannot finish at the full-rate baseline"
    );
    assert_eq!(
        serde_json::to_value(&world).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    advance_to(&mut world, started + 1199);
    advance_to(&mut restored, started + 1199);
    assert_eq!(cuts(&world, runway), 1);
    harpoon_core::sim::tick(&mut world, &[]);
    harpoon_core::sim::tick(&mut restored, &[]);
    assert_eq!(cuts(&world, runway), 0);
    assert_eq!(
        serde_json::to_value(&world).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    assert_eq!(
        world.entity(runway).unwrap().hp,
        90.,
        "cut repair does not regenerate structural DP"
    );
}

#[test]
fn an_in_progress_bound_job_pauses_when_its_declared_reduction_becomes_unknown() {
    let (mut world, base, runway, shop) = fixture(true, None);
    assert_binding(&world, runway);
    order(&mut world, base, runway);
    let started = world.tick;
    advance_to(&mut world, started + 200);
    disable_provider(&mut world, shop);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    advance_to(&mut world, started + 1200);
    advance_to(&mut restored, started + 1200);
    assert_eq!(
        cuts(&world, runway),
        1,
        "an old full-rate due tick cannot complete through unknown provider loss"
    );
    assert_eq!(
        world.rules.ground.as_ref().unwrap().repair_jobs.len(),
        1,
        "unfinished suspended work must survive"
    );
    assert_eq!(
        serde_json::to_value(&world).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn actual_bound_work_changes_rate_at_provider_loss_and_trusted_restoration_without_free_work() {
    let (mut world, base, runway, shop) = fixture(true, Some(0.5));
    assert_binding(&world, runway);
    let healthy_facility = world.rules.ground.as_ref().unwrap().facilities[&shop].clone();
    let healthy_entity = world.entity(shop).unwrap().clone();
    order(&mut world, base, runway);
    let started = world.tick;
    advance_to(&mut world, started + 200);
    disable_provider(&mut world, shop);
    advance_to(&mut world, started + 300);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    advance_to(&mut world, started + 400);
    advance_to(&mut restored, started + 400);
    assert_eq!(cuts(&world, runway), 1);
    // Trusted already-adjudicated fixture restoration only. There is currently
    // no native building-repair producer, and this test does not claim one.
    for state in [&mut world, &mut restored] {
        state
            .rules
            .ground
            .as_mut()
            .unwrap()
            .facilities
            .insert(shop, healthy_facility.clone());
        state.entities.insert(shop, healthy_entity.clone());
        assert!(ground::operational(state, shop));
    }
    // 200 units full-rate + 200*0.5 + 300 full-rate = 600 actual work units.
    advance_to(&mut world, started + 699);
    advance_to(&mut restored, started + 699);
    assert_eq!(cuts(&world, runway), 1);
    harpoon_core::sim::tick(&mut world, &[]);
    harpoon_core::sim::tick(&mut restored, &[]);
    assert_eq!(cuts(&world, runway), 0);
    assert_eq!(
        serde_json::to_value(&world).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn explicit_provider_must_identify_an_actual_own_maintenance_facility() {
    let (base, _, _, shop) = fixture(true, Some(0.5));
    for name in ["", "MISSING", "BASE", "RUNWAY"] {
        let mut world = base.clone();
        world
            .context
            .ground_2026
            .as_mut()
            .unwrap()
            .facilities
            .get_mut("RUNWAY")
            .unwrap()
            .runway
            .as_mut()
            .unwrap()
            .repair
            .as_mut()
            .unwrap()
            .maintenance_provider = Some(name.into());
        assert!(
            ground::initialize(&mut world).is_err(),
            "invalid provider accepted: {name}"
        );
    }
    let mut foreign = base.clone();
    foreign.entity_mut(shop).unwrap().side = Side::Red;
    assert!(ground::initialize(&mut foreign).is_err());
    let mut ambiguous = base.clone();
    let id = ambiguous.allocate_id();
    let mut duplicate = ambiguous.entity(shop).unwrap().clone();
    duplicate.id = id;
    ambiguous.entities.insert(id, duplicate);
    assert!(ground::initialize(&mut ambiguous).is_err());
}

#[test]
fn optional_binding_and_work_fields_preserve_unbound_serialized_shape() {
    let historical_spec =
        json!({"seconds_per_cut":600,"teams":1,"source":"scenario_assumption: historical fixture"});
    let spec: ground::RepairSpec = serde_json::from_value(historical_spec.clone()).unwrap();
    assert_eq!(serde_json::to_value(spec).unwrap(), historical_spec);
    let (mut world, base, runway, _) = fixture(false, None);
    order(&mut world, base, runway);
    let historical_job = json!({"runway":runway,"due":world.tick+600,"owner":"Blue"});
    assert_eq!(
        serde_json::to_value(&world.rules.ground.as_ref().unwrap().repair_jobs[0]).unwrap(),
        historical_job
    );
    let restored: World = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&world).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn repeated_same_tick_ground_steps_do_not_grant_bound_repair_work() {
    let (mut world, base, runway, shop) = fixture(true, Some(0.5));
    disable_provider(&mut world, shop);
    order(&mut world, base, runway);
    assert_eq!(
        world.rules.ground.as_ref().unwrap().repair_jobs[0]
            .work
            .as_ref()
            .unwrap()
            .completed_seconds,
        0.
    );
    ground::step(&mut world);
    ground::step(&mut world);
    assert_eq!(
        world.rules.ground.as_ref().unwrap().repair_jobs[0]
            .work
            .as_ref()
            .unwrap()
            .completed_seconds,
        0.
    );
    harpoon_core::sim::tick(&mut world, &[]);
    assert_eq!(
        world.rules.ground.as_ref().unwrap().repair_jobs[0]
            .work
            .as_ref()
            .unwrap()
            .completed_seconds,
        0.5
    );
    ground::step(&mut world);
    assert_eq!(
        world.rules.ground.as_ref().unwrap().repair_jobs[0]
            .work
            .as_ref()
            .unwrap()
            .completed_seconds,
        0.5
    );
}

#[test]
fn a_destroyed_bound_provider_cannot_apply_its_partial_damage_rate_to_old_or_new_jobs() {
    let (mut world, base, runway, shop) = fixture(true, Some(0.5));
    order(&mut world, base, runway);
    let started = world.tick;
    advance_to(&mut world, started + 200);
    let total = ground::source_target(&world, shop)
        .unwrap()
        .damage_points
        .unwrap();
    ground::apply_damage(
        &mut world,
        shop,
        total,
        0,
        "scenario_assumption: adjudicated complete shop destruction",
    )
    .unwrap();
    assert!(world.rules.ground.as_ref().unwrap().facilities[&shop].destroyed);
    // While its wreck entity still exists, the source gate specifically
    // rejects the destroyed provider before even considering busy teams.
    let before_rejection = world.event_seq;
    order(&mut world, base, runway);
    assert_eq!(world.rules.ground.as_ref().unwrap().repair_jobs.len(), 1);
    assert_new_repair_rejection(&world, base, before_rejection, "destroyed");
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    advance_to(&mut world, started + 2000);
    advance_to(&mut restored, started + 2000);
    assert_eq!(cuts(&world, runway), 1);
    let work = world.rules.ground.as_ref().unwrap().repair_jobs[0]
        .work
        .as_ref()
        .unwrap();
    assert_eq!(work.completed_seconds, 200.);
    assert!(work.last_rate.is_none());
    assert!(work.pending.is_some());
    assert_eq!(
        serde_json::to_value(&world).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    // Remove the suspended job only in this isolated referee fixture to reach
    // the new-order gate without the unrelated all-teams-busy rejection.
    world.rules.ground.as_mut().unwrap().repair_jobs.clear();
    // Platform wrecks are removed by the normal 120s cleanup. The explicit
    // named provider can no longer resolve to an entity; this is an earlier
    // source-identity rejection, not the still-live wreck's "destroyed" gate.
    assert!(world.entity(shop).is_none());
    let before_rejection = world.event_seq;
    order(&mut world, base, runway);
    assert!(world.rules.ground.as_ref().unwrap().repair_jobs.is_empty());
    assert_new_repair_rejection(
        &world,
        base,
        before_rejection,
        "must identify one actual facility",
    );
}

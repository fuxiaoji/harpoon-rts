//! ZCODE-FACILITY-20261030: author-declared parking routes consuming the
//! printed taxiway-cut isolation. Root wires the shared parent field; this
//! file only uses public APIs.
//!
//! Source (personally viewed 2026-10-30, English 2026-01-01 PDF118/11-3
//! "Generic Land Targets", Taxiways row): Size C, no damage points, armor 0,
//! "Cut isolates aircraft from runways". The printed rule is the isolation
//! FUNCTION only — no duration, area or probability is printed anywhere — so
//! which parking area serves which base/class is an explicit scenario author
//! declaration (`ground_2026.parking`, this file's `route` JSON), never
//! inferred, and never a real-world site or per-airframe claim.
//!
//! The declared taxiway cut below is an explicitly adjudicated boundary
//! (applied through the public ground2026::apply_damage with a named
//! identity): it is NOT a natural hit and no die is claimed.
//!
//! NOT COMPILED, NOT RUN by this task (no Cargo); root runs these after
//! wiring the fixed ABI into ground2026.

use harpoon_core::commands::AirMission;
use harpoon_core::rules::ground2026::{self as ground};
use harpoon_core::types::PlatformKind;
use harpoon_core::{Database, Scenario, World};
use serde_json::{json, Value};

/// Routes JSON injected into `ground_2026.parking` (root-owned field).
fn route(base: &str, class: &str, area: &str) -> Value {
    json!({"source":"scenario_assumption: exercise deployment routes, not real-world sites or per-airframe tracking",
        "base":base,"class":class,"parking_area":area,"scope":"all_airframes_of_class"})
}

fn config(routes: Value) -> Value {
    json!({"source":"scenario_assumption: exercise parking route declaration","routes":routes})
}

/// One Blue exercise airfield: Base (service + air wing), Runway, Park
/// (routed parking area, no DP) and TwyA whose cut isolates Park.
fn input(parking: Value, extra_units: &mut Vec<Value>) -> Value {
    let mut units = json!([
        {"side":"Blue","class":"airfield","name":"Base","pos":[0,0]},
        {"side":"Blue","class":"airfield","name":"Runway","pos":[0,0]},
        {"side":"Blue","class":"airfield","name":"Park","pos":[0,0]},
        {"side":"Blue","class":"airfield","name":"TwyA","pos":[0,0]},
        {"side":"Red","class":"cn_052d","name":"DISTANT_OTHER_SIDE","pos":[1000,1000],"fire_on":false}]);
    units.as_array_mut().unwrap().extend(extra_units.drain(..));
    json!({
     "id":"facility-parking-routes-lab","name":"scenario_assumption: 11.5.1 parking route lab",
     "seed":7,"duration_s":5000,
     "weather":{"visibility_nm":10,"jam":[0,0]},
     "context":{"rules_profile":"hv2026-rts-v4",
      "air_wings":{"Base":[{"class":"us_f18e","count":2}]},
      "aviation_2026":{"source":"scenario_assumption: exercise load declaration",
       "loads":{"us_f18e":{"payload_kg":0,"tanks":[],"source":"scenario_assumption: clean exercise load"}},"parking":{"Base":0}},
      "ground_2026":{"source":"scenario_assumption: explicit exercise airfield",
       "facilities":{
         "Runway":{"template":"runway","source":"English2026 PDF118/11-3, visually verified","parent":"Base",
          "runway":{"length_m":3000,"baseline":"Large"}},
         "Park":{"template":"taxiway","source":"English2026 PDF118/11-3 Generic Land Targets Taxiways row, visually verified 2026-10-30; author parking area, no DP","parent":"Base"},
         "TwyA":{"template":"taxiway","source":"English2026 PDF118/11-3 Generic Land Targets Taxiways row, visually verified 2026-10-30","parent":"Base",
          "taxiway":{"isolated_parking":["Park"]}}},
       "air_services":{"Base":{
         "source":"scenario_assumption: RTS airfield envelopes only",
         "launch_lanes":1,"landing_lanes":1,"launch_interval_s":60,"landing_interval_s":60,
         "heading_deg":0,"initial_speed_kts":{"us_f18e":200},
         "landing_speed_kts":{"us_f18e":200},"turnaround_s":{"us_f18e":600},
         "landing_min_altitude_m":61,"landing_max_altitude_m":2000,
         "landing_speed_tolerance_kts":1,"landing_heading_tolerance_deg":1}},
       "parking":parking}},
     "units":units})
}

fn load(scenario_json: Value) -> Result<World, String> {
    let scenario: Scenario = serde_json::from_value(scenario_json).unwrap();
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    harpoon_core::load_scenario(scenario, db)
}

fn id_of(w: &World, name: &str) -> harpoon_core::EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}

fn aircraft_count(w: &World) -> usize {
    w.entities
        .values()
        .filter(|e| e.kind == PlatformKind::Aircraft)
        .count()
}

fn queue_one(w: &mut World) -> Result<(), String> {
    let base = id_of(w, "Base");
    ground::queue_launch(w, base, "us_f18e", 1, &AirMission::Rtb, None)
}

/// Adjudicated boundary (declared, not a natural hit): the public damage API
/// disables TwyA; with no damage points its disabled flag needs no die.
fn declare_taxiway_cut(w: &mut World) {
    let twy = id_of(w, "TwyA");
    ground::apply_damage(w, twy, 1.0, 0, "declared-taxiway-cut-boundary")
        .expect("the declared cut is a public adjudicated input");
    let f = &w.rules.ground.as_ref().unwrap().facilities[&twy];
    assert!(f.disabled, "the declared cut must disable the taxiway");
}

/// A routed base whose parking area is cut refuses the actual public launch
/// command, and the refused command leaves the whole World untouched.
#[test]
fn cut_routed_parking_area_refuses_queue_launch_with_unchanged_payload() {
    let mut extra = Vec::new();
    let mut w = load(input(config(json!([route("Base","us_f18e","Park")])), &mut extra)).expect("valid routes must load");
    declare_taxiway_cut(&mut w);
    let before = serde_json::to_vec(&w).unwrap();
    let err = queue_one(&mut w).unwrap_err();
    assert!(err.contains("isolates"), "{err}");
    assert_eq!(
        before,
        serde_json::to_vec(&w).unwrap(),
        "a refused launch must not touch lanes, queues or stock"
    );
    let state = w.rules.ground.as_ref().unwrap();
    assert!(state.launches.is_empty());
}

/// The gate is live at execution time too: a launch queued before the cut
/// stays queued (no drop, no second stock debit) while the route is blocked,
/// and the identical undeclared-cut flow launches — the block is caused by
/// the cut, not by queue corruption. No public un-cut exists, so the cleared
/// route is shown by the same fixture without the declared cut.
#[test]
fn cut_after_queuing_keeps_the_job_queued_without_double_debit() {
    let mut extra = Vec::new();
    let mut w = load(input(config(json!([route("Base","us_f18e","Park")])), &mut extra)).expect("valid routes must load");
    queue_one(&mut w).expect("an intact routed parking area must accept the launch");
    assert_eq!(w.rules.ground.as_ref().unwrap().launches.len(), 1);
    declare_taxiway_cut(&mut w);
    let mut restored: World =
        serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 0..310 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap(),
        "the blocked queue must stay save/restore consistent without hidden stock movement"
    );
    let state = w.rules.ground.as_ref().unwrap();
    assert_eq!(aircraft_count(&w), 0, "a cut route must not release the queued flight");
    assert_eq!(state.launches.len(), 1, "the job stays queued, never dropped");

    // Same fixture and queue without the declared cut: the flow continues.
    let mut extra = Vec::new();
    let mut clear = load(input(config(json!([route("Base","us_f18e","Park")])), &mut extra)).expect("valid routes must load");
    queue_one(&mut clear).expect("intact route");
    for _ in 0..310 {
        harpoon_core::sim::tick(&mut clear, &[]);
    }
    let diag:Vec<_>=clear.events.iter().filter_map(|e|match &e.kind {harpoon_core::EventKind::RulesEvent{rule,detail,..} if rule.contains("launch")=>Some(format!("{rule}: {detail}")),_=>None}).collect();
    assert_eq!(aircraft_count(&clear), 1, "an intact routed flow launches: {diag:?}");
    assert!(clear.rules.ground.as_ref().unwrap().launches.is_empty());
    assert!(clear.events.iter().any(|e| matches!(&e.kind,
        harpoon_core::EventKind::RulesEvent { rule, .. } if rule == "11.airfield_launch_complete")));
}

/// Invalid author declarations are refused at load: unknown base, duplicate
/// base/class route, parking area without the explicit parent, cross-side
/// area, non-aircraft class, base without a declared air service and a
/// non-assumption source.
#[test]
fn invalid_parking_route_declarations_are_refused_at_load() {
    let mut extra = Vec::new();
    extra.push(json!({"side":"Red","class":"airfield","name":"RedPark","pos":[9,9]}));
    extra.push(json!({"side":"Blue","class":"airfield","name":"LooseArea","pos":[5,5]}));
    extra.push(json!({"side":"Blue","class":"airfield","name":"Outfield","pos":[7,7]}));
    let mut extra2 = extra.clone();
    let cases = vec![
        ("unknown base", json!([route("NOWHERE","us_f18e","Park")])),
        ("duplicate base/class", json!([route("Base","us_f18e","Park"),route("Base","us_f18e","Park")])),
        ("area without explicit parent", json!([route("Base","us_f18e","LooseArea")])),
        ("cross-side area", json!([route("Base","us_f18e","RedPark")])),
        ("class is not aircraft", json!([route("Base","cn_052d","Park")])),
        ("base without air service", json!([route("Outfield","us_f18e","Park")])),
    ];
    for (label, routes) in cases {
        let scenario_json = input(config(routes), &mut extra.clone());
        assert!(
            load(scenario_json).is_err(),
            "{label} must be refused at load"
        );
    }
    let mut untyped = config(json!([route("Base","us_f18e","Park")]));
    untyped["routes"][0]["source"] = json!("author private note without assumption label");
    assert!(
        load(input(untyped, &mut extra2)).is_err(),
        "a route without a scenario_assumption source must be refused"
    );
}

#[test]
fn foreign_taxiway_cannot_declare_isolation_of_an_owned_parking_route() {
    let mut extra=vec![json!({"side":"Red","class":"airfield","name":"ForeignBase","pos":[20,0]}),
        json!({"side":"Red","class":"airfield","name":"ForeignTwy","pos":[20,0]})];
    let mut v=input(config(json!([route("Base","us_f18e","Park")])),&mut extra);
    let mut twy=v["context"]["ground_2026"]["facilities"]["TwyA"].clone();
    twy["parent"]=json!("ForeignBase");v["context"]["ground_2026"]["facilities"]["ForeignTwy"]=twy;
    assert!(load(v).is_err(),"a foreign/other-base taxiway cannot own a declared route to this base's parking area");
}
#[test]
fn isolated_recovery_has_no_alignment_lane_or_reservation_mutation_after_restore() {
    let mut extra=vec![json!({"side":"Blue","class":"us_f18e","name":"INCOMING","pos":[0,0],"altitude_m":4000,"course_deg":90,"speed_kts":250,"fire_on":false})];
    let mut w=load(input(config(json!([route("Base","us_f18e","Park")])),&mut extra)).unwrap();
    declare_taxiway_cut(&mut w);
    let plane=id_of(&w,"INCOMING");let base=id_of(&w,"Base");
    let before=serde_json::to_value(&w).unwrap();
    assert!(ground::recovery_ready(&mut w,plane,base).is_none());
    assert_eq!(before,serde_json::to_value(&w).unwrap(),"blocked routed recovery must not turn/descend/reserve before gate");
    let mut restored:World=serde_json::from_value(before.clone()).unwrap();
    assert!(ground::recovery_ready(&mut restored,plane,base).is_none());
    assert_eq!(before,serde_json::to_value(&restored).unwrap());
}

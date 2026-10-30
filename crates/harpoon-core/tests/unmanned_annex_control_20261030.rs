//! Annex B printed control-field qualification (public behavior examples).
//! Source: Americas Aircraft 1 Dec 25, MQ-4C Triton B-65/66 and MQ-25A
//! Stingray B-67, pages visually reviewed 2026-10-30 (SHA256 c5f29a4b…c83b1).
//! The annex opt-in certifies only the printed CONTROL facts; operational
//! choices (initial mode, controller/element, loss behavior, satellite
//! availability), antenna geometry and endurance are explicit scenario_assumption
//! author inputs; no payload/equipment/endurance qualification is claimed and
//! no contact/FCS/classification/HP/die/state injection is used anywhere.
//! Real DB classes are used read-only; the original DB/PDF are untouched.
//! NOT COMPILED AND NOT RUN in this batch (cargo prohibited by task); written
//! against the root-synced parent (VehicleFit.annex_platform, `pub mod annex`,
//! validate call sites in unmanned2026 initialize/config_for and launch
//! validate_catalog). Root compiles and runs this leaf with the batch.
use harpoon_core::rules::unmanned2026 as u;
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

fn id(w: &World, name: &str) -> EntityId {
    w.entities
        .values()
        .find(|e| e.name == name)
        .unwrap()
        .id
}

fn database() -> Database {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    db.validate().unwrap();
    db
}

/// Author scenario fits. Everything operational here is scenario_assumption:
/// element naming, initial mode/controller choice, home, antenna geometry and
/// national satellite bandwidth. `annex_platform` is the only source-certified
/// part and only for the control facts printed on the annex pages.
fn controller_fit(los_nm: Option<f64>) -> Value {
    let mut f = json!({
        "source": "scenario_assumption: author control element; national satellite bandwidth is an explicit scenario asset",
        "elements": ["ctrl"],
        "operational": true,
        "satellite": true,
        "satellite_available": true
    });
    if let Some(los) = los_nm {
        // Author geometry, not a book value; the printed 150 nmi cap is the
        // vehicle-side hardware fact, the controller figure is the author's
        // scenario equipment choice.
        f["los_range_nm"] = json!(los);
        f["antenna_height_m"] = json!(10.0);
    }
    f
}

fn vehicle_fit(class: &str, link: &str, los_nm: Option<f64>) -> Value {
    let mut f = json!({
        "source": "scenario_assumption: operational choices only; certified control fields via annex_platform",
        "annex_platform": class,
        "domain": "air",
        "autonomous_capable": false,
        "initial_mode": "hitl",
        "controller": "CTRL",
        "element": "ctrl",
        "initial_link": link,
        "satellite": true,
        "satellite_available": true,
        "loss_behavior": {"type": "return_to_home", "home": "CTRL"}
    });
    if let Some(los) = los_nm {
        f["los_range_nm"] = json!(los);
        // Author geometry required by the engine LOS gate; never sourced from
        // the book, which prints no antenna height for these platforms.
        f["antenna_height_m"] = json!(10.0);
    }
    f
}

fn annex_world(seed: u64, class: &str, vehicle_name: &str, fit: Value, ctrl: Value) -> Result<World, String> {
    let config = json!({
        "source": "scenario_assumption: Annex B control-field qualification experiment",
        "controllers": {"CTRL": ctrl},
        "vehicles": {vehicle_name: fit}
    });
    let sc: Scenario = serde_json::from_value(json!({
        "id": "annex-control-20261030",
        "name": "scenario_assumption: Annex B printed control-field experiment, original DB hulls/stocks only",
        "duration_s": 4000,
        "seed": seed,
        "weather": {"visibility_nm": 0, "jam": [0, 0]},
        "context": {"rules_profile": "hv2026-rts-v4", "unmanned_2026": config},
        "units": [
            {"side": "Blue", "class": "us_burke_2a", "name": "CTRL", "pos": [0, 0], "course_deg": 90, "speed_kts": 0, "fire_on": false},
            // Vehicle offset: 0.2 deg (~12 nmi) is far inside every gate below;
            // 0.3 deg (~18 nmi) stays inside the printed 150 nmi cap and the
            // shared radar horizon at the altitudes used here.
            {"side": "Blue", "class": class, "name": vehicle_name, "pos": [0.2, 0], "course_deg": 90, "speed_kts": 310, "altitude_m": 8000, "fire_on": false}
        ]
    }))
    .unwrap();
    harpoon_core::load_scenario(sc, database())
}

fn gate_set_speed(w: &World, unit: EntityId, speed_kts: f64) -> Result<(), String> {
    Command::SetSpeed { unit, speed_kts }.validate_source(w, Side::Blue)
}

/// 1) Real DB us_mq4c under printed SATCOM control: a direct tactical order
///    flows through the existing can_control -> link_with gate while the
///    author-supplied national bandwidth is available; withdrawing it
///    restricts the actual controlled command at the same public gate and
///    restoring it behaves identically again.
#[test]
fn mq4c_satcom_control_and_bandwidth_loss_restore() {
    let mut w = annex_world(
        20261030,
        "us_mq4c",
        "TRITON",
        vehicle_fit("us_mq4c", "satellite", None),
        controller_fit(None),
    )
    .unwrap();
    let (triton, ctrl) = (id(&w, "TRITON"), id(&w, "CTRL"));

    // Printed SATCOM control, author bandwidth present: real command accepted
    // and actually ordered.
    gate_set_speed(&w, triton, 300.).unwrap();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, Command::SetSpeed { unit: triton, speed_kts: 300. })]);
    assert_eq!(w.entity(triton).unwrap().ordered_speed_kts, 300.);

    // Referee withdraws the author-declared national asset: the same direct
    // order is refused at the source gate (no partial application).
    u::set_satellite_available(&mut w, &[triton, ctrl], false, "scenario_assumption: national bandwidth withdrawn").unwrap();
    let err = gate_set_speed(&w, triton, 310.).unwrap_err();
    assert!(err.contains("2.5 satellite"), "unexpected gate error: {err}");
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(w.entity(triton).unwrap().ordered_speed_kts, 300., "rejected order must not change the ordered speed");

    // Restore: identical command behaves as before the outage.
    u::set_satellite_available(&mut w, &[triton, ctrl], true, "scenario_assumption: national bandwidth restored").unwrap();
    let take=u::Action::TakeControl{controller:ctrl,element:"ctrl".into(),link:u::ControlLink::Satellite};
    u::validate_action(&w,Side::Blue,triton,&take).unwrap();
    let cmd:Command=serde_json::from_value(json!({"cmd":"unmanned","unit":triton,"action":take})).unwrap();
    harpoon_core::sim::tick(&mut w,&[(Side::Blue,cmd)]);
    let mut restored:World=serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
    for _ in 0..180 {harpoon_core::sim::tick(&mut w,&[]);harpoon_core::sim::tick(&mut restored,&[]);}
    assert_eq!(serde_json::to_value(&w).unwrap(),serde_json::to_value(restored).unwrap());
    gate_set_speed(&w, triton, 310.).unwrap();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, Command::SetSpeed { unit: triton, speed_kts: 310. })]);
    assert_eq!(w.entity(triton).unwrap().ordered_speed_kts, 310.);
}

/// 2) Real DB us_mq25a with the printed 150 nmi LOS hardware cap: a fit at
///    exactly the printed cap is accepted and carries a real order inside the
///    shared radar-horizon gate; one tenth over the printed cap is refused at
///    initialization. Payload stays exactly the original DB fit (empty), and
///    the certification makes no payload claim.
#[test]
fn mq25a_printed_los_cap_boundary_and_payload_untouched() {
    // Real LOS consumer at author scenario distance inside the printed cap.
    let mut w = annex_world(
        20261031,
        "us_mq25a",
        "STINGRAY",
        vehicle_fit("us_mq25a", "line_of_sight", Some(150.0)),
        controller_fit(Some(150.0)),
    )
    .unwrap_or_else(|e| panic!("fit at the printed 150 nmi cap must initialize: {e}"));
    // Normalized class offsets are too large for the altitude/horizon gate at
    // 8000 m? The vehicle was spawned at [0.2,0]; keep the asserted order well
    // inside every gate by using cruise speed at the same position.
    let stingray = id(&w, "STINGRAY");
    gate_set_speed(&w, stingray, 380.).unwrap();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, Command::SetSpeed { unit: stingray, speed_kts: 380. })]);
    assert_eq!(w.entity(stingray).unwrap().ordered_speed_kts, 380.);

    // One tenth of a nmi beyond the printed hardware cap: refused at init.
    let err = annex_world(
        20261032,
        "us_mq25a",
        "STINGRAY",
        vehicle_fit("us_mq25a", "line_of_sight", Some(150.1)),
        controller_fit(Some(150.0)),
    )
    .unwrap_err();
    assert!(
        err.contains("exceeds the printed us_mq25a cap of 150"),
        "unexpected init error: {err}"
    );

    // Payload untouched: the certification adds no ordnance to the DB row or
    // to the spawned entity; the annex scope never covers payload.
    let db = database();
    assert!(db.platforms.get("us_mq25a").unwrap().weapons.is_empty());
    assert!(w.entity(stingray).unwrap().weapons.is_empty());
}

/// 3) Annex opt-in rejections: name/class mismatch, unknown printed row,
///    unprinted autonomy, unprinted acoustic modem, and every way of lending
///    MQ-4C a LOS link the page never printed.
#[test]
fn annex_rejections() {
    let base = |class: &str, fit: Value| {
        annex_world(
            20261033,
            class,
            "TRITON",
            fit,
            controller_fit(None),
        )
    };
    let mut wrong_class = vehicle_fit("us_mq4c", "satellite", None);
    wrong_class["annex_platform"] = json!("us_mq25a");
    let err = base("us_mq4c", wrong_class).unwrap_err();
    assert!(err.contains("does not match actual class us_mq4c"), "{err}");

    let mut unknown_row = vehicle_fit("us_mq4c", "satellite", None);
    unknown_row["annex_platform"] = json!("us_global_hawk");
    let err = base("us_mq4c", unknown_row).unwrap_err();
    assert!(err.contains("Annex B"), "{err}");

    let mut autonomy = vehicle_fit("us_mq4c", "satellite", None);
    autonomy["autonomous_capable"] = json!(true);
    let err = base("us_mq4c", autonomy).unwrap_err();
    assert!(err.contains("autonomy is not printed"), "{err}");

    let mut modem = vehicle_fit("us_mq4c", "satellite", None);
    modem["acoustic_modem"] = json!(true);
    let err = base("us_mq4c", modem).unwrap_err();
    assert!(err.contains("acoustic modem is not printed"), "{err}");

    // MQ-4C: the page prints SATCOM only — neither a LOS number nor a LOS
    // link can be borrowed under the annex name.
    let err = base("us_mq4c", vehicle_fit("us_mq4c", "satellite", Some(100.0))).unwrap_err();
    assert!(err.contains("prints no LOS control-link range"), "{err}");
    let err = base("us_mq4c", vehicle_fit("us_mq4c", "line_of_sight", None)).unwrap_err();
    assert!(err.contains("cannot be used by name"), "{err}");
}

/// 4) `annex_platform: None` keeps the legacy author fit path unchanged: the
///    same real DB class still initializes and takes direct orders without
///    any annex certification.
#[test]
fn legacy_none_annex_fit_unchanged() {
    let mut fit = vehicle_fit("us_mq4c", "satellite", None);
    fit.as_object_mut().unwrap().remove("annex_platform");
    let mut w = annex_world(20261034, "us_mq4c", "TRITON", fit, controller_fit(None)).unwrap();
    let triton = id(&w, "TRITON");
    gate_set_speed(&w, triton, 300.).unwrap();
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, Command::SetSpeed { unit: triton, speed_kts: 300. })]);
    assert_eq!(w.entity(triton).unwrap().ordered_speed_kts, 300.);
}

//! Independent candidates, not registered or run by this author.
//! English2026 PDF137/14-11: a real ship must be stationary and upwind.
//! All fits/complements below are explicit experiment parameters. Native
//! sink_by is an adjudicated loss-consumer boundary, not a weapon engagement.
//! No survivor, Contact, FCS, RNG result, casualty or clock is written here.
use harpoon_core::rules::expanded_damage2026 as ex;
use harpoon_core::{Command, Database, EntityId, Scenario, Side, World};
use serde_json::{json, Value};

fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}

fn fixture(rescue_class: &str) -> Result<World, String> {
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .map_err(|e| e.to_string())?;
    // Restrict only experiment receivers; original hull/DP/weapons stay intact.
    db.platforms.get_mut("us_ford").unwrap().sensors = vec!["visual".into()];
    for class in ["us_burke_2a", "cn_093", "airfield"] {
        db.platforms.get_mut(class).unwrap().sensors.clear();
    }
    let sc: Scenario = serde_json::from_value(json!({
        "id":"independent-sar-recovery-guards-20261026",
        "name":"scenario_assumption: actual platform kind and loss guard laboratory",
        "seed":20261026,"duration_s":1000,
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{
            "rules_profile":"hv2026-rts-v4",
            "flight_environment":{"wind_from_deg":270,"wind_kts":3},
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "expanded_damage_2026":{
                "source":"scenario_assumption: optional SAR, laboratory complements and station radius, not actual Annex fits",
                "optional":{"search_and_rescue":true},
                "units":{
                    "LOOK":{"source":"scenario_assumption: lookout only"},
                    "RESCUE":{
                        "source":"scenario_assumption: experimental rescue complement, no actual equipment claim",
                        "crew":100,
                        "rescue":{"source":"scenario_assumption: declared500m rescue station and100-person capacity","kind":"ship","capacity":100,"station_radius_m":500}
                    },
                    "LOST":{"source":"scenario_assumption: ten-person complement for unambiguous1-9 survivor sighting","crew":10}
                }
            },
            "detection_2026":{"environment":{
                "source":"scenario_assumption: clear daytime open water without terrain/cloud/ice",
                "sea_state":0,"precipitation":0,"visibility_percent":100,
                "shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null
            }}
        },
        "units":[
            {"side":"Blue","class":"us_ford","name":"LOOK","pos":[0,0],"course_deg":0,"speed_kts":0,"fire_on":false},
            {"side":"Blue","class":rescue_class,"name":"RESCUE","pos":[0,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"us_burke_2a","name":"LOST","pos":[0.01,0],"course_deg":0,"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"DUMMY","pos":[2000,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}
        ]
    }))
    .map_err(|e| e.to_string())?;
    let mut w = harpoon_core::load_scenario(sc, db)?;
    let lost = id(&w, "LOST");
    harpoon_core::damage::sink_by(&mut w, lost, Side::Blue);
    assert!(ex::survivor_view(&w, id(&w, "LOOK")).is_empty());
    // T1 normal sensor phase performs the successful close-range native scan.
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(w.tick, 1);
    let reports = ex::survivor_view(&w, id(&w, "LOOK"));
    assert_eq!(reports.len(), 1, "real surface lookout must discover the loss group");
    assert_eq!(reports[0]["estimated_group_size"], "less_than_ten");
    let label = reports[0]["label"].as_str().unwrap();
    let command: Command = serde_json::from_value(json!({
        "cmd":"search_and_rescue","unit":id(&w,"RESCUE"),
        "action":{"type":"begin","group":label,"automatic":true}
    }))
    .map_err(|e| e.to_string())?;
    // This is the normal public shared command path, at T2.
    harpoon_core::sim::tick(&mut w, &[(Side::Blue, command)]);
    assert_eq!(w.tick, 2);
    Ok(w)
}

fn advance_to(w: &mut World, to: i64) {
    while w.tick < to {
        let old = w.tick;
        harpoon_core::sim::tick(w, &[]);
        assert_eq!(w.tick, old + 1, "fixture must not end before its guard boundary");
    }
}

fn own(w: &World, unit: EntityId) -> Value {
    ex::own_view(w, unit).expect("Full source own projection")
}

fn aboard(w: &World, unit: EntityId) -> u64 {
    own(w, unit)["survivors_aboard"].as_u64().unwrap_or(0)
}

fn total_recovered(w: &World) -> u64 {
    // Read only, using JSON rather than depending on a new private Rust field.
    let v = serde_json::to_value(&w.rules.expanded_damage).unwrap();
    v["survivors"].as_object().unwrap().values()
        .map(|g| g["rescued"].as_u64().unwrap())
        .sum()
}

#[test]
fn r01_actual_live_stationary_upwind_ship_is_a_nonvacuous_recovery_control() {
    let mut w = fixture("us_burke_2a").unwrap();
    let rescuer = id(&w, "RESCUE");
    assert!(own(&w, rescuer)["rescue"].is_object(), "public Begin must actually apply");
    assert_eq!(aboard(&w, rescuer), 0);
    // Begin T2 +180s means the first eligible automatic Tac boundary is T360.
    advance_to(&mut w, 359);
    assert_eq!(aboard(&w, rescuer), 0);
    harpoon_core::sim::tick(&mut w, &[]);
    assert_eq!(w.tick, 360);
    assert!(aboard(&w, rescuer) > 0, "source D6/2 in sea0 rescues at least five if available");
    assert_eq!(total_recovered(&w), aboard(&w, rescuer));
}

#[test]
fn r02_native_sunk_rescuer_cannot_recover_automatically_while_its_wreck_lingers() {
    let mut w = fixture("us_burke_2a").unwrap();
    let rescuer = id(&w, "RESCUE");
    assert!(own(&w, rescuer)["rescue"].is_object());
    advance_to(&mut w, 359);
    assert_eq!(aboard(&w, rescuer), 0);
    harpoon_core::damage::sink_by(&mut w, rescuer, Side::Red);
    assert!(!w.entity(rescuer).unwrap().alive);
    let before = total_recovered(&w);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    harpoon_core::sim::tick(&mut w, &[]);
    harpoon_core::sim::tick(&mut restored, &[]);
    assert_eq!(w.tick, 360);
    assert!(w.entity(rescuer).is_some(), "the native120s wreck window is still open");
    assert_eq!(total_recovered(&w), before, "a dead automatic rescuer must not pick anyone up");
    assert_eq!(aboard(&w, rescuer), 0);
    assert_eq!(serde_json::to_vec(&w).unwrap(), serde_json::to_vec(&restored).unwrap());
}

fn mismatched_ship_fit_cannot_recover(actual_class: &str) {
    // Either initialization must reject this declared wrong-kind fit, or the
    // real command/consumer must fail closed without taking actual survivors.
    let mut w = match fixture(actual_class) {
        Ok(w) => w,
        Err(reason) => {
            assert!(reason.to_ascii_lowercase().contains("rescue") || reason.contains("14.8"),
                "unrelated loader failure is not the tested source-kind guard: {reason}");
            return;
        }
    };
    let rescuer = id(&w, "RESCUE");
    assert_ne!(w.entity(rescuer).unwrap().kind, harpoon_core::types::PlatformKind::Ship);
    advance_to(&mut w, 360);
    assert_eq!(aboard(&w, rescuer), 0, "declaring ship rescue must not turn another physical kind into a surface ship");
    assert_eq!(total_recovered(&w), 0);
}

#[test]
fn r03_actual_base_with_mismatched_ship_rescue_fit_cannot_pick_up_survivors() {
    mismatched_ship_fit_cannot_recover("airfield");
}

#[test]
fn r04_actual_submarine_with_mismatched_ship_rescue_fit_cannot_pick_up_survivors() {
    mismatched_ship_fit_cannot_recover("cn_093");
}

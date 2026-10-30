//! Source PDF79 / printed7-4. Native launches, no authored flight or RNG state.
//! Author Good/contacts/platform mounts isolate post-release mechanics only.
use harpoon_core::{
    load_scenario, Command, ContactId, Database, EntityId, EventKind, Scenario, Side, Vec2, World,
};
use serde_json::{json, Value};
use std::sync::OnceLock;

pub fn events(w: &World, wanted: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == wanted => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
pub fn fixture(
    separation: f64,
    count: usize,
    mixed: bool,
    enable_nm: f64,
    unequal_depth: bool,
) -> (World, Vec<EntityId>, EntityId) {
    fixture_options(
        separation,
        count,
        mixed,
        enable_nm,
        unequal_depth,
        None,
        false,
    )
}
pub fn fixture_options(
    separation: f64,
    count: usize,
    mixed: bool,
    enable_nm: f64,
    unequal_depth: bool,
    lab_range: Option<f64>,
    late_third: bool,
) -> (World, Vec<EntityId>, EntityId) {
    fixture_setup(
        separation,
        count,
        mixed,
        enable_nm,
        unequal_depth,
        lab_range,
        late_third,
        |_| {},
    )
}
pub fn fixture_setup(
    separation: f64,
    count: usize,
    mixed: bool,
    enable_nm: f64,
    unequal_depth: bool,
    lab_range: Option<f64>,
    late_third: bool,
    setup: impl FnOnce(&mut Value),
) -> (World, Vec<EntityId>, EntityId) {
    static DB: OnceLock<Database> = OnceLock::new();
    let db = DB
        .get_or_init(|| {
            Database::from_json(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../database/consolidated.json"
            )))
            .unwrap()
        })
        .clone();
    let classes = ["us_virginia", "us_seawolf", "us_ohio_ssgn"];
    let mut platforms = json!({});
    let mut orders = json!({});
    let mut units = Vec::new();
    for n in 0..count {
        let weapon = if mixed && n == 1 {
            "us_mk54"
        } else {
            "us_mk48"
        };
        let name = format!("LAUNCHER{n}");
        let depth = if unequal_depth && n == 1 { 100. } else { 50. };
        let enable = if late_third && n == 2 { 1. } else { enable_nm };
        let position = if late_third && n == 2 {
            separation / 2.
        } else {
            separation * n as f64
        };
        platforms[classes[n]] = json!({"source":"scenario_assumption: independent controlled tube/mount binding, not a real complete hull fit",
            "wire_capacity":2,"reload_parallel":2,"tubes":[{"bearing_deg":0,"loaded":weapon}],"reserves":{}});
        orders[format!("{name}:{weapon}")] = json!({"source":"scenario_assumption: explicit isolated post-release order",
            "speed_index":0,"depth_m":depth,"enable_run_nm":enable,"bearing_deg":0.,"cut_wire":true,
            "spread_deg":[],"solution":{"quality":"Good","source":"scenario_assumption: author Good, not native FCS"}});
        units.push(
            json!({"side":"Blue","class":classes[n],"name":name,"pos":[position,0.],
            "course_deg":0,"speed_kts":0,"depth_m":depth,"fire_on":false}),
        );
    }
    units.push(json!({"side":"Red","class":"cn_093","name":"ORIGINAL","pos":[0,8],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}));
    let mut scenario_value = json!({"id":"interference-source-20261025","name":"scenario_assumption native pair",
        "duration_s":3000,"seed":42,"weather":{"visibility_nm":0,"jam":[0,0]},
        "context":{"rules_profile":"hv2026-rts-v4","torpedo_2026":{"source":"scenario_assumption: qualified weapon definitions and isolated geometry",
            "environment":{"source":"scenario_assumption no layer or reverberation","layer_m":null,"reverberant":false},
            "platforms":platforms,"orders":orders,
            "targets":{"cn_093":{"source":"scenario_assumption: known original signature","size":2,"noise":2,"anechoic":false,"foil_borne":false}}}},"units":units});
    if let Some(range) = lab_range {
        let mut weapon = harpoon_core::rules::torpedo2026::tables()["weapons"]["us_mk48"].clone();
        weapon["source"]=json!("scenario_assumption: deliberately shortened laboratory endurance, not real Mk48 performance");
        for setting in weapon["settings"].as_array_mut().unwrap() {
            setting["range_nm"] = json!(range);
        }
        scenario_value["context"]["torpedo_2026"]["weapons"] = json!({"us_mk48":weapon});
    }
    setup(&mut scenario_value);
    let scenario: Scenario = serde_json::from_value(scenario_value).unwrap();
    let mut w = load_scenario(scenario, db).unwrap();
    let original = w
        .entities
        .values()
        .find(|e| e.name == "ORIGINAL")
        .unwrap()
        .id;
    let shooters: Vec<_> = (0..count)
        .map(|n| {
            w.entities
                .values()
                .find(|e| e.name == format!("LAUNCHER{n}"))
                .unwrap()
                .id
        })
        .collect();
    for (n, id) in shooters.iter().enumerate() {
        let mount = w.entity_mut(*id).unwrap().weapons.first_mut().unwrap();
        mount.weapon = if mixed && n == 1 {
            "us_mk54"
        } else {
            "us_mk48"
        }
        .into();
        mount.ammo = 2;
        mount.mounts = 1;
        mount.cooldown_s = 0.;
    }
    for e in w.entities.values_mut() {
        e.sensors.clear();
        if e.id == original {
            e.weapons.clear();
        }
    }
    w.sides[0].contacts.insert(
        ContactId(99),
        harpoon_core::world::Contact {
            id: ContactId(99),
            about: original,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 8.),
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
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", original.0),
        harpoon_core::types::PlatformKind::Sub,
    );
    let commands: Vec<_> = shooters
        .iter()
        .enumerate()
        .map(|(n, id)| {
            (
                Side::Blue,
                Command::Fire {
                    unit: *id,
                    contact: ContactId(99),
                    weapon: if mixed && n == 1 {
                        "us_mk54"
                    } else {
                        "us_mk48"
                    }
                    .into(),
                    rounds: 1,
                },
            )
        })
        .collect();
    harpoon_core::sim::tick(&mut w, &commands);
    let bodies: Vec<_> = w
        .entities
        .values()
        .filter(|e| e.kind == harpoon_core::types::PlatformKind::Torpedo && e.alive)
        .map(|e| e.id)
        .collect();
    assert_eq!(
        bodies.len(),
        count,
        "all independent physical tube loads must actually release; events={:?}",
        w.events
    );
    for body in &bodies {
        assert!(w
            .rules
            .torpedoes
            .as_ref()
            .unwrap()
            .launches
            .contains_key(body));
    }
    (w, bodies, original)
}
pub fn until(w: &mut World, tick: i64) {
    while w.tick < tick {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
pub fn actual_lock(w: &mut World) {
    until(w, 3);
    assert_eq!(
        events(w, "7.5.3.same_type_interference").len(),
        2,
        "two real nonempty private lock receipts"
    );
}

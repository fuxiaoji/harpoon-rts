//! Wide-batch unrun nuclear-ASW source candidates. Native reports/FCS and real carrier bodies
//! are required; no Contact/FCS/HP/die/Flight state is injected.
use harpoon_core::rules::{asw_weapons2026 as a};
use harpoon_core::{Command, ContactId, Database, EntityId, EventKind, Scenario, Side, World};
use serde_json::json;

const ACTIVE_SONAR: &str = "sonar_a_asw_lab_active";
const PASSIVE_SONAR: &str = "asw_lab_passive";
// Explicit laboratory value: C-size strength0 + broadside1 + stationary0 +
// sea-state0/layer0 gives +1, so 16 * 1.25 = the printed 20nm centre row.
// The old 20nm experiment gave 25nm, tied between printed 24/26nm rows;
// the production source ambiguity guard correctly refused that input.
const ACTIVE_RANGE_NM: f64 = 16.0;

fn fixture(seed: u64, enabled: bool) -> World {
    fixture_launcher(seed, enabled, false)
}
fn fixture_launcher(seed: u64, enabled: bool, submarine_launcher: bool) -> World {
    let launcher = if submarine_launcher {
        "us_virginia"
    } else {
        "us_ford"
    };
    // Both original hulls have independently sourced combat-system entries.
    // The VLA mount and the sonar installation remain explicit laboratory fits.
    let sonar_mount = if submarine_launcher {
        "aft_digital"
    } else {
        "bow"
    };
    let context = json!({"rules_profile":"hv2026-rts-v4","nuclear_2026":{"source":"scenario_assumption: explicit nuclear ASW laboratory; not real VLA warhead","units":{"SENDER":{"us_vlasroc":{"source":"scenario_assumption: declared experimental depth-bomb payload","source_variant":"scenario_assumption: nuclear ASW consumer laboratory","allowed_yields_kt":[20],"selected_yield_kt":20,"medium":"subsurface","fuze":"free_fall","authorized":true}}}},"radar_environment":{"sea_state":0,"precipitation":"None"},
        "asw_weapons_2026":if enabled{Some(json!({"source":"scenario_assumption: explicit laboratory carrier/payload fit, Annex E4 transport values","standoffs":{"us_vlasroc":{"source":"scenario_assumption: loaded carrier + Mk54 payload declaration; no real platform fit completeness claim","minimum_range_nm":2.5,"maximum_range_nm":12.0,"carrier_speed_kts":660.0,"guided":true,"payload":{"type":"nuclear","yield_kt":20}}}}))}else{None},
        "torpedo_2026":{"source":"scenario_assumption: explicit payload environment and target signature only","environment":{"source":"scenario_assumption: deep open water","layer_m":null,"reverberant":false},"targets":{"cn_093":{"source":"scenario_assumption: size/noise/coating experiment","size":2,"noise":2,"anechoic":false,"foil_borne":false}}},
        "detection_2026":{"bindings":{
            format!("{launcher}:{ACTIVE_SONAR}"):{"type":"sonar","source":"scenario_assumption: laboratory active Gen5 LMF16nm, bow on surface/aft_digital on submerged experiment; no real hardware claim","generation":5,"passive_range_nm":null,"active_range_nm":ACTIVE_RANGE_NM,"frequency":"lmf","mount":sonar_mount,"active_torpedo_detection":false,"high_speed_tow":false},
            format!("{launcher}:{PASSIVE_SONAR}"):{"type":"sonar","source":"scenario_assumption: independent laboratory passive Gen5 LMF16nm, bow on surface/aft_digital on submerged experiment, for native held-contact classification; no real hardware claim","generation":5,"passive_range_nm":ACTIVE_RANGE_NM,"active_range_nm":null,"frequency":"lmf","mount":sonar_mount,"active_torpedo_detection":false,"high_speed_tow":false}},
            "environment":{"source":"scenario_assumption: open water","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null},
            "targets":{"cn_093":{"source":"scenario_assumption: controlled target signature","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}}}});
    let sc:Scenario=serde_json::from_value(json!({"id":"asw-native-resume","name":"scenario_assumption: native ASW experiment","seed":seed,"duration_s":5000,"weather":{"visibility_nm":0,"jam":[0,0]},"context":context,"units":[
        {"side":"Blue","class":launcher,"name":"SENDER","pos":[0,0],"course_deg":if submarine_launcher{180}else{0},"speed_kts":if submarine_launcher{3}else{0},"depth_m":if submarine_launcher{50}else{0},"sonar_active":true,"fire_on":false},
        {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,3],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    let mut db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    for (sensor_id, kind) in [
        (ACTIVE_SONAR, harpoon_core::db::SensorKind::SonarActive),
        (PASSIVE_SONAR, harpoon_core::db::SensorKind::SonarPassive),
    ] {
        let mut sensor = db.sensors.values().next().unwrap().clone();
        sensor.id = sensor_id.into();
        sensor.name = format!("scenario_assumption: {sensor_id} receiver");
        sensor.kind = kind;
        sensor.air_range_nm = 0.0;
        sensor.surface_range_nm = 0.0;
        sensor.missile_range_nm = 0.0;
        sensor.sub_range_nm = ACTIVE_RANGE_NM;
        db.sensors.insert(sensor.id.clone(), sensor);
    }
    // Keep original hull DP/stocks. Native passive LMF held-contact processing
    // earns Sub classification after a Tactical epoch; active LMF supplies the
    // independent measured fix/FCS. An active echo alone cannot earn that type.
    db.platforms.get_mut(launcher).unwrap().sensors =
        vec![ACTIVE_SONAR.into(), PASSIVE_SONAR.into()];
    db.platforms.get_mut("cn_093").unwrap().sensors.clear();
    // Explicit experiment only: put the original Burke mount, with its actual
    // stock, on the source-CS Ford/Virginia hull. This does not claim that either
    // real platform carries VLA-ASROC. The submarine begins at its legal3kt
    // release speed, heading away; its declared aft receivers keep real contact.
    let mount = db.platforms["us_burke_2a"]
        .weapons
        .iter()
        .find(|m| m.weapon == "us_vlasroc")
        .unwrap()
        .clone();
    db.platforms.get_mut(launcher).unwrap().weapons.push(mount);
    db.validate().unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let actor = id(&w, "SENDER");
    assert!(
        w.entity(actor)
            .unwrap()
            .sensors
            .iter()
            .any(|s| s.sensor == ACTIVE_SONAR && s.active),
        "actual loaded laboratory sonar must start active"
    );
    assert!(w
        .rules
        .detection
        .as_ref()
        .unwrap()
        .config
        .bindings
        .contains_key(&format!("{launcher}:{ACTIVE_SONAR}")));
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Blue,
            Command::SetSonar {
                unit: actor,
                active: true,
            },
        )],
    );
    run(&mut w, 725);
    w
}
fn id(w: &World, name: &str) -> EntityId {
    w.entities.values().find(|e| e.name == name).unwrap().id
}
fn run(w: &mut World, to: i64) {
    while w.tick < to {
        assert!(!w.over);
        harpoon_core::sim::tick(w, &[]);
    }
}
fn contact(w: &World) -> ContactId {
    let target = id(w, "TARGET");
    w.sides[0]
        .contacts
        .values()
        .find(|c| c.about == target && !c.stale && !c.bearing_only)
        .unwrap_or_else(|| {
            let traces: Vec<_> = w
                .events
                .iter()
                .filter_map(|e| match &e.kind {
                    EventKind::RulesEvent { rule, detail, .. } if rule == "5.scan" => Some(detail),
                    _ => None,
                })
                .collect();
            panic!("native located contact required; actual sensor scan evidence: {traces:?}")
        })
        .id
}
fn fire(w: &mut World) {
    let actor = id(w, "SENDER");
    let c = contact(w);
    a::validate_fire(w, actor, c, "us_vlasroc").expect("native source FCS checkpoint required");
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: actor,
                contact: c,
                weapon: "us_vlasroc".into(),
                rounds: 1,
            },
        )],
    );
}
fn rounds(w: &World) -> u16 {
    w.entity(id(w, "SENDER"))
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_vlasroc")
        .unwrap()
        .ammo
}


#[test]
fn n01_actual_native_nuclear_asw_release_executes_placement_then_its_qualified_primary_effect() {
    let mut w=fixture(20261025,true);
    let shooter=id(&w,"SENDER");let target=id(&w,"TARGET");let c=contact(&w);
    assert_eq!(harpoon_core::rules::detection2026::known_category(&w,Side::Blue,target),Some(harpoon_core::types::PlatformKind::Sub));
    assert!(a::validate_fire(&w,shooter,c,"us_vlasroc").is_ok(),"source-qualified native nuclear payload is an executable mechanism, not a blanket pending guard");
    let stock=rounds(&w);fire(&mut w);assert_eq!(rounds(&w),stock-1);
    let end=w.tick+60;
    while w.tick<end&&!w.over {harpoon_core::sim::tick(&mut w,&[]);}
    let flight=w.rules.asw_weapons.as_ref().unwrap().flights.values().next().unwrap();
    assert!(flight.terminal&&flight.placement.is_some()&&flight.payload_position.is_some());
    let placement=flight.placement.as_ref().unwrap();
    assert!(placement.placed,"this native near-range Good depth-bomb placement is the kill control");
    let function=w.events.iter().find_map(|e|match &e.kind {
        EventKind::RulesEvent{rule,detail,..}if rule=="14.1.8.detonation"=>Some(serde_json::from_str::<serde_json::Value>(detail).unwrap()),_=>None
    }).expect("one actual nuclear function event after physical placement");
    assert_eq!(w.events.iter().filter(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..}if rule=="14.1.8.detonation")).count(),1);
    if function["failed"]==true {assert!(w.entity(target).unwrap().alive);}else{assert!(!w.entity(target).unwrap().alive,"successful placement is the special nuclear-ASW kill, independent of an invented ordinary Pk");}
    let saved=serde_json::to_vec(&w).unwrap();let restored:World=serde_json::from_slice(&saved).unwrap();
    assert_eq!(serde_json::to_vec(&restored).unwrap(),saved);
}

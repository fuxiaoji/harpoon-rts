//! Source12.6.2 corrective candidate: actual deployed MobileDecoy -> normal
//! installed Ka-27/ru_mad scan -> native report/side contact. Artifact only.
//! All experimental fits precede load; no private runtime/Contact/die injection.
use harpoon_core::db::SensorKind;
use harpoon_core::rules::{detection2026 as d, ew2026 as ew};
use harpoon_core::types::PlatformKind;
use harpoon_core::{Command, Database, EntityId, EventKind, Scenario, Side, Visibility, World};
use serde_json::{json, Value};

fn seed() -> u64 {
    // Select a reproducible original trial before loading, never replace a die
    // in the live World. With two initial bodies, the actual deploy allocates3.
    (0..10_000)
        .find(|seed| d::trial(&mut d::State::default(), *seed, 1, "5.7.1:1:ru_mad:3", 180).0 == 1)
        .expect("bounded original-D1001 fixture seed")
}
fn fixture(generation: u8) -> World {
    let fit = format!("lab_mobile_gen{generation}");
    let input = json!({"id":"mobile-mad-source-correction","name":"actual Ka27 MAD and author mobile simulator; no real Virginia decoy fit claim","seed":seed(),"duration_s":600,
      "weather":{"visibility_nm":0,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4",
      "radar_environment":{"sea_state":0,"precipitation":"None"},
      "ew_2026":{"fits":{(fit.clone()):{"source":"scenario_assumption: generation-matched internal mobile simulator and finite launch stock, not a named Annex model","kind":"mobile_decoy","generation":generation,"launcher":"internal","endurance_s":null,"deployment_s":null,"radar_height_m":null}},"loadouts":{"us_virginia":{(fit):1}},"environment":{"source":"scenario_assumption: open calm deep-water experiment","sea_state":0,"wind_from_deg":0,"wind_speed_kts":0}},
      "detection_2026":{"bindings":{"ru_ka27:ru_mad":{"type":"mad","source":"scenario_assumption: existing actually installed ru_mad; nonwestern ordinary receiver with explicitly authored0.5nm experiment envelope, not Annex performance certification","range_nm":0.5,"western":false}},
      "targets":{"us_virginia":{"source":"scenario_assumption: explicit size/noise signature imitated by the launched simulator, not inferred Annex coverage","size":2,"noise":2,"anechoic":false,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":2}},
      "environment":{"source":"scenario_assumption: controlled clear straight/level actual MAD pass","sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":3000,"layer":null,"ice":0,"cloud":null}}},
      "units":[{"side":"Blue","class":"ru_ka27","name":"SEARCH","pos":[-0.2,0],"course_deg":90,"speed_kts":60,"altitude_m":200,"fire_on":false},
      {"side":"Red","class":"us_virginia","name":"OWNER","pos":[0,0],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false}]});
    let mut db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    assert_eq!(db.sensors["ru_mad"].kind, SensorKind::Magnetic);
    assert!(db.platforms["ru_ka27"]
        .sensors
        .iter()
        .any(|s| s == "ru_mad"));
    // Retain real fitted hardware; never clone a different sensor and relabel
    // its kind. Remove other detection and attack equipment before load only.
    db.platforms
        .get_mut("ru_ka27")
        .unwrap()
        .sensors
        .retain(|s| s == "ru_mad");
    db.platforms.get_mut("ru_ka27").unwrap().weapons.clear();
    db.platforms.get_mut("us_virginia").unwrap().sensors.clear();
    db.platforms.get_mut("us_virginia").unwrap().weapons.clear();
    let w = harpoon_core::load_scenario(serde_json::from_value::<Scenario>(input).unwrap(), db)
        .unwrap();
    assert_eq!(w.entity(EntityId(1)).unwrap().kind, PlatformKind::Aircraft);
    assert_eq!(w.entity(EntityId(2)).unwrap().kind, PlatformKind::Sub);
    assert!(w.sides.iter().all(|s| s.contacts.is_empty()));
    assert!(w.rules.ew.as_ref().unwrap().objects.is_empty());
    w
}
fn deploy(g: u8) -> Command {
    serde_json::from_value(json!({"cmd":"electronic_warfare","unit":2,
       "action":{"type":"deploy","fit":format!("lab_mobile_gen{g}"),"at":null,"radar_signature":null,"course_deg":0,"speed_kts":([7.,10.,12.,15.][g as usize-1]),"depth_m":50}})).unwrap()
}
fn scan_result(w: &World, target: EntityId) -> Value {
    w.events
        .iter()
        .rev()
        .find_map(|e| match &e.kind {
            EventKind::RulesEvent {
                rule,
                unit: Some(observer),
                detail,
                ..
            } if rule == "5.scan" && *observer == EntityId(1) => {
                let v: Value = serde_json::from_str(detail).unwrap();
                (v["sensor"] == "ru_mad" && v["truth_target"] == target.0)
                    .then(|| v["result"].clone())
            }
            _ => None,
        })
        .expect(
            "normal native Detection emits a referee scan trace for the actual deployed simulator",
        )
}
#[test]
fn mobile_mad01_actual_gen3_and4_deploys_return_to_installed_native_mad_with_gen1_and2_negative_controls(
) {
    // The known old negative/positive controls execute before the actual Gen3
    // counterexample, so a correct control cannot be hidden by that red.
    for g in [1, 2, 4, 3] {
        let mut w = fixture(g);
        harpoon_core::sim::tick(&mut w, &[(Side::Red, deploy(g))]);
        let obj = w
            .rules
            .ew
            .as_ref()
            .unwrap()
            .objects
            .values()
            .next()
            .expect("normal command really deployed a mobile simulator");
        let id = obj.id;
        assert_eq!(
            id,
            EntityId(3),
            "original seed tags the actually allocated object"
        );
        assert_eq!(obj.kind, ew::ObjectKind::MobileDecoy);
        assert_eq!(obj.generation, Some(g));
        assert_eq!(obj.status, ew::Status::Deployed);
        assert!(
            !w.entities.contains_key(&id),
            "normal sensor-only EW target is removed after scanning, never a combat body"
        );
        assert_eq!(
            ew::own_view(&w, Side::Red, EntityId(2)).unwrap()["stocks"]
                [format!("lab_mobile_gen{g}")],
            0,
            "the actual finite simulator launch stock is paid"
        );
        let native = scan_result(&w, id);
        let contact = w.find_contact(Side::Blue, id);
        assert_eq!(contact.is_some(), g>=3,
            "English2026 PDF121/12-3 and Chinese source tables: MAD Capability Gen1/2 No, Gen3/4 Yes; actual Gen{g} native result={native}");
        assert_eq!(
            ew::tables()["mobile_submarine_decoys"]["generations"][g as usize - 1]["mad"].as_bool(),
            Some(g >= 3),
            "structured source capability must agree with the actually consumed native result"
        );
        let key = format!("1:ru_mad:{}", id.0);
        let report = w
            .rules
            .aaw
            .as_ref()
            .unwrap()
            .firecontrol
            .tracks
            .get(&key)
            .and_then(|t| t.report.as_ref());
        if g >= 3 {
            assert_eq!(native["rule"], "5.7.1");
            assert_eq!(
                native["d100"].as_u64(),
                Some(1),
                "the normal original D100 was not overridden"
            );
            assert!(native["probability"].as_f64().unwrap() >= 1.);
            let c = w.contact(Side::Blue, contact.unwrap()).unwrap();
            assert_eq!(c.category, PlatformKind::Sub);
            assert!(
                c.class_name.is_none(),
                "MAD cannot reveal the private simulator generation or true identity"
            );
            let r = report.expect(
                "real successful native SensorReport is consumed by the local evidence recorder",
            );
            assert_eq!(r.observer, EntityId(1));
            assert_eq!(r.target, id);
            assert_eq!(r.sensor, "ru_mad");
            assert_eq!(r.sensor_kind, SensorKind::Magnetic);
            assert_eq!(r.at, 1);
            assert!(w.events.iter().any(|e| e.vis==Visibility::Side(Side::Blue)
                && matches!(&e.kind,EventKind::Detection {contact:cid,sensor,..} if Some(*cid)==contact && sensor=="ru_mad")),
                "normal own-side Detection receipt, not a private Contact insertion");
        } else {
            assert_eq!(native["rule"], "12.sensor_return");
            assert!(
                report.is_none(),
                "Gen1/2 cannot acquire native MAD evidence"
            );
        }
        let old_pos = w.rules.ew.as_ref().unwrap().objects[&id].pos;
        let mut replay: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut replay, &[]);
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            serde_json::to_value(&replay).unwrap(),
            "normal motion and native evidence survive FullWorld restore for every generation"
        );
        assert!(old_pos.range_to(w.rules.ew.as_ref().unwrap().objects[&id].pos) > 0.);
        assert!(!w.entities.contains_key(&id));
    }
}

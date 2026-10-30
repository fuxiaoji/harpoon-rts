//! Source PDF114/10-5 §10.2.7: opportunity targets after a failed
//! reacquisition — MOSS attacked through the Torpedo-ACMs column and destroyed
//! on a hit, other real units through the same Placement procedure, nearest
//! candidate first (RTS-8).
use harpoon_core::{
    load_scenario, Command, ContactId, Database, EventKind, Scenario, Side, Vec2, World,
};
use serde_json::{json, Value};

const TORP: u32 = 4;
const MOSS: u32 = 3;

fn roll(seed: u64, entity: u32, purpose: &str, sides: u8) -> u8 {
    let mut r =
        harpoon_core::Rng::new(seed).derive(harpoon_core::Rng::hash_tag(0, entity, purpose));
    1 + (r.next_f64() * sides as f64) as u8
}
fn d100(seed: u64, purpose: &str) -> u8 {
    roll(seed, TORP, purpose, 100)
}
fn d10(seed: u64, purpose: &str) -> u8 {
    roll(seed, 0, purpose, 10)
}
fn attack_purpose(pass: u32) -> String {
    format!("10.2.5.attack:{pass}")
}
fn placement_purpose(pass: u32) -> String {
    format!("10.placement:{TORP}:{pass}")
}
fn op_purpose(pass: u32, candidate: u32) -> String {
    format!("10.placement:{TORP}:{pass}:op:{candidate}")
}
fn seed_for(test: impl Fn(u64) -> bool) -> u64 {
    (1..=2_000_000)
        .find(|seed| test(*seed))
        .expect("pre-World native seed exists")
}

fn fixture(seed: u64, moss: bool) -> (World, harpoon_core::EntityId, harpoon_core::EntityId) {
    let db = Database::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/consolidated.json"
    )))
    .unwrap();
    let sc: Scenario = serde_json::from_value(json!({
        "id":"torpedo-opportunity","name":"10.2.7 opportunity chain","duration_s":3000,"seed":seed,
        "context":{"rules_profile":"hv2026-rts-v4",
        "detection_2026":{"source":"scenario_assumption: reattack environment","targets":{"cn_093":{"source":"scenario_assumption: signature the decoy imitates","size":2,"noise":2,"anechoic":true,"titanium":false,"pumpjet":false,"stealthy_ir":false,"radar_signature":0}},"environment":{"source":"scenario_assumption: open water 400m",
            "sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,"water_depth_m":400.0,"layer":null,"ice":0,"cloud":null}},
        "torpedo_2026":{"source":"scenario_assumption: opportunity chain mechanisms",
        "environment":{"source":"scenario_assumption: open water no layer","layer_m":null,"reverberant":false},
        "targets":{"cn_093":{"source":"scenario_assumption: controlled target","size":2,"noise":2,"anechoic":true,"foil_borne":false}},
        "platforms":{"us_virginia":{"source":"scenario_assumption: loaded tubes","wire_capacity":2,"reload_parallel":2,
            "tubes":[{"bearing_deg":0,"loaded":"us_mk48"},{"bearing_deg":0,"loaded":"us_mk48"}],"reserves":{"us_mk48":2}}},
        "orders":{"SHOOTER:us_mk48":{"source":"scenario_assumption: declared order","speed_index":0,"depth_m":50,
            "enable_run_nm":0,"bearing_deg":null,"cut_wire":true,"spread_deg":[],
            "solution":{"quality":"Good","source":"scenario_assumption: fixture solution"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_virginia","name":"SHOOTER","pos":[0,0],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"TARGET","pos":[0,5.0],"speed_kts":0,"depth_m":50,"fire_on":false}]})).unwrap();
    let mut w = load_scenario(sc, db).unwrap();
    let s = w
        .entities
        .values()
        .find(|e| e.name == "SHOOTER")
        .unwrap()
        .id;
    let t = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    w.entity_mut(t).unwrap().weapons.clear();
    if moss {
        // Deploy a real Red-side mobile decoy through the EW command path.
        use harpoon_core::rules::ew2026 as ew;
        let mut f = ew::Fit {
            source: "scenario_assumption: generic mechanism fixture".into(),
            kind: ew::ObjectKind::MobileDecoy,
            generation: Some(3),
            launcher: Some(ew::Launcher::Internal),
            endurance_s: None,
            deployment_s: None,
            radar_height_m: Some(1.),
        };
        f.generation = Some(3);
        {
            let st = w.rules.ew.as_mut().unwrap();
            st.config.fits.insert("mobile".into(), f.clone());
            st.config
                .loadouts
                .entry("cn_093".into())
                .or_default()
                .insert("mobile".into(), 2);
        }
        let launcher = t;
        w.entity_mut(launcher).unwrap().kind = harpoon_core::types::PlatformKind::Sub;
        let speed = [7., 10., 12., 15.][2];
        ew::apply(
            &mut w,
            launcher,
            &ew::Action::Deploy {
                fit: "mobile".into(),
                at: None,
                radar_signature: None,
                course_deg: Some(90.),
                speed_kts: Some(speed),
                depth_m: Some(50.),
            },
        )
        .expect("real Red MOSS deployment");
    }
    w.sides[0].contacts.insert(
        ContactId(99),
        harpoon_core::world::Contact {
            id: ContactId(99),
            about: t,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 5.0),
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
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert("0:2".into(), harpoon_core::types::PlatformKind::Sub);
    (w, s, t)
}

fn launch(w: &mut World, s: harpoon_core::EntityId) -> harpoon_core::EntityId {
    harpoon_core::sim::tick(
        w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: s,
                contact: ContactId(99),
                weapon: "us_mk48".into(),
                rounds: 1,
            },
        )],
    );
    w.entities
        .values()
        .find(|e| e.kind == harpoon_core::types::PlatformKind::Torpedo)
        .expect("torpedo released")
        .id
}

fn details(w: &World, rule: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RulesEvent {
                rule: r, detail, ..
            } if r == rule => Some(serde_json::from_str(detail).unwrap()),
            _ => None,
        })
        .collect()
}

fn moss_seed(last_hit: bool) -> u64 {
    seed_for(|seed| {
        d10(seed, &placement_purpose(0)) <= 7
            && d100(seed, &attack_purpose(0)) > 64
            && d10(seed, &placement_purpose(1)) > 4
            && matches!(d10(seed, &op_purpose(1, MOSS)), 5 | 6)
            && (d100(seed, &attack_purpose(1)) <= 64) == last_hit
    })
}

fn await_moss(w: &mut World) {
    for _ in 0..2200 {
        harpoon_core::sim::tick(w, &[]);
        if !details(w, "10.2.7.opportunity_acquired").is_empty()
            || w.rules
                .pending_adjudications
                .contains_key(&harpoon_core::EntityId(TORP))
        {
            break;
        }
    }
}

#[test]
fn native_moss_captures_without_inherited_coating_then_hits_acm_column() {
    let (mut w, shooter, original) = fixture(moss_seed(true), true);
    let torpedo = launch(&mut w, shooter);
    await_moss(&mut w);
    let candidate = details(&w, "10.2.4.opportunity_placement");
    assert_eq!(candidate.len(), 1, "one qualified native MobileDecoy");
    assert_eq!(candidate[0]["candidate"], MOSS);
    assert_eq!(
        candidate[0]["placement"]["chance"], 6,
        "Fair no-coating MOSS is6; original coated target chance4 is not inherited"
    );
    assert_eq!(candidate[0]["placement"]["placed"], true);
    let f = &w.rules.torpedoes.as_ref().unwrap().flights[&torpedo];
    let capture = f.opportunity.as_ref().unwrap().capture.as_ref().unwrap();
    assert_eq!(f.target, original);
    assert_eq!(f.placement.as_ref().unwrap().chance, 4);
    assert_eq!(capture.captured_at, w.tick);
    assert_eq!(
        capture.pursuit_started_at, None,
        "capture tick has no extra pursuit"
    );
    assert!(!w.rules.pending_adjudications.contains_key(&torpedo));
    for _ in 0..1200 {
        if !w.entity(torpedo).is_some_and(|e| e.alive) {
            break;
        }
        let body = w.entity(torpedo).unwrap();
        let moss = &w.rules.ew.as_ref().unwrap().objects[&harpoon_core::EntityId(MOSS)];
        let before = body.pos.range_to(moss.pos);
        let f = &w.rules.torpedoes.as_ref().unwrap().flights[&torpedo];
        let setting = &f.weapon.settings[f
            .opportunity
            .as_ref()
            .unwrap()
            .capture
            .as_ref()
            .unwrap()
            .attack_speed_index];
        // MOSS moves before weapon pursuit, so include its maximum actual
        // one-second closure bound when deciding that a hit is impossible.
        let reachable = (setting.speed_kts + moss.speed_kts) / 3600.;
        let n = details(&w, "10.2.5.source_terminal").len();
        harpoon_core::sim::tick(&mut w, &[]);
        if before > reachable + 1e-9 {
            assert_eq!(
                details(&w, "10.2.5.source_terminal").len(),
                n,
                "no terminal from farther than the complete relative-motion bound"
            );
        }
    }
    let attacks = details(&w, "10.2.5.source_terminal");
    assert_eq!(attacks.len(), 2);
    assert_eq!(attacks[1]["probability"], 0.64);
    assert_eq!(attacks[1]["opportunity_kind"], "mobile_decoy");
    assert_eq!(attacks[1]["hit"], true);
    assert_eq!(
        w.rules.ew.as_ref().unwrap().objects[&harpoon_core::EntityId(MOSS)].status,
        harpoon_core::rules::ew2026::Status::Destroyed
    );
    assert_eq!(details(&w, "10.2.7.opportunity_destroyed").len(), 1);
    assert!(!w.entity(torpedo).is_some_and(|e| e.alive));
    let own = harpoon_core::rules::torpedo2026::own_view(&w, Side::Blue, shooter).unwrap();
    assert_eq!(own["releases"][0]["status"], "unknown");
    assert!(own["releases"][0]["physical_alive"].is_null());
    assert!(!own.to_string().contains("opportunity_acquired"));
}

#[test]
fn native_moss_miss_preserves_one_die_and_pending_instead_of_returning_to_original() {
    let (mut w, shooter, original) = fixture(moss_seed(false), true);
    let torpedo = launch(&mut w, shooter);
    await_moss(&mut w);
    assert_eq!(details(&w, "10.2.7.opportunity_acquired").len(), 1);
    for _ in 0..1200 {
        harpoon_core::sim::tick(&mut w, &[]);
        if w.rules.pending_adjudications.contains_key(&torpedo) {
            break;
        }
    }
    assert!(w.rules.pending_adjudications.contains_key(&torpedo));
    let old = details(&w, "10.2.5.source_terminal");
    assert_eq!(old.len(), 2);
    assert_eq!(old[1]["hit"], false);
    assert_eq!(old[1]["probability"], 0.64);
    let original_die = old[1]["d100"].clone();
    let saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let mut restored = saved;
    for _ in 0..15 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    let f = &w.rules.torpedoes.as_ref().unwrap().flights[&torpedo];
    assert_eq!(f.target, original);
    assert!(f
        .opportunity
        .as_ref()
        .unwrap()
        .capture
        .as_ref()
        .unwrap()
        .terminal
        .is_some());
    assert_eq!(
        details(&w, "10.2.5.source_terminal").len(),
        2,
        "no retry attack die while pending"
    );
    assert_eq!(
        details(&w, "10.2.5.source_terminal")[1]["d100"],
        original_die
    );
    assert!(details(&w, "10.2.7.opportunity_destroyed").is_empty());
}

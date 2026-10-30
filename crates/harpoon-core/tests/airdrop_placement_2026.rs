//! R-054: English Jan2026 PDF109/9-8,74/6-6,79/7-4,113-114/10-4--5.
//! Real Fire -> water entry -> source placement -> immediate source attack.
//! Only MH60R BlockI/four Mk54 is source-exact. The reported position, Good/
//! Fair/Poor solution, water and target acoustic properties are explicit
//! scenario assumptions. These fixtures are not a complete real ASW FCS fit.
use harpoon_core::rules::{aviation2026 as av, firecontrol2026::Placement, torpedo2026 as torp};
use harpoon_core::{
    Command, ContactId, Database, EntityId, EventKind, Rng, Scenario, Side, Vec2, Visibility, World,
};
use serde_json::{json, Value};
use std::sync::OnceLock;

fn fixture(
    quality: &str,
    coating: bool,
    two: bool,
) -> (World, EntityId, EntityId, Option<EntityId>) {
    static DB: OnceLock<Database> = OnceLock::new();
    let db = DB
        .get_or_init(|| {
            Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
        })
        .clone();
    let mut orders = serde_json::Map::new();
    for name in if two {
        vec!["HELO1", "HELO2"]
    } else {
        vec!["HELO1"]
    } {
        orders.insert(format!("{name}:us_mk54"), json!({
            "source":"scenario_assumption: declared source test order", "speed_index":0,
            "depth_m":50,"enable_run_nm":0,"bearing_deg":null,"spread_deg":[],"cut_wire":true,
            "solution":{"quality":quality,"source":"scenario_assumption: controlled placement quality, not fused contact quality"}
        }));
    }
    let mut units = vec![
        json!({"side":"Blue","class":"us_mh60r_mk54x4","name":"HELO1","pos":[0,0],"speed_kts":0,"altitude_m":1000,"fire_on":false}),
        json!({"side":"Red","class":"cn_093","name":"TARGET","pos":[0,0.25],"course_deg":90,"speed_kts":0,"depth_m":50,"fire_on":false}),
    ];
    if two {
        units.push(json!({"side":"Blue","class":"us_mh60r_mk54x4","name":"HELO2","pos":[0,0],"speed_kts":0,"altitude_m":1000,"fire_on":false}));
    }
    let sc: Scenario = serde_json::from_value(json!({
        "id":"r054-entry","name":"R054 controlled entry placement","duration_s":1000,"seed":20261024,
        "context":{"rules_profile":"hv2026-rts-v4",
            "aviation_2026":{"source":"US Aircraft2025 PDF69/B64 exact BlockI four-Mk54 fit; E1 noteC helicopter285kg",
                "loads":{"us_mh60r_mk54x4":{"payload_kg":1140,"tanks":[],"external_weapon_rounds":{"us_mk54":4},"source":"US Aircraft2025 PDF69/B64 and PDF93/E1 noteC"}},
                "parking":{},"initial_fuel_nm":{"HELO1":200,"HELO2":200}},
            "torpedo_2026":{"source":"scenario_assumption: controlled placement, water and target attributes",
                "environment":{"source":"scenario_assumption: no layer or reverberation","layer_m":null,"reverberant":false},
                "targets":{"cn_093":{"source":"scenario_assumption: controlled target acoustics, not a verified complete093 fit","size":2,"noise":2,"anechoic":coating,"foil_borne":false}},
                "orders":orders}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":units
    })).unwrap();
    let mut w = harpoon_core::load_scenario(sc, db).unwrap();
    let h = w.entities.values().find(|e| e.name == "HELO1").unwrap().id;
    let t = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    let h2 = w
        .entities
        .values()
        .find(|e| e.name == "HELO2")
        .map(|e| e.id);
    // Disable unrelated sensor/weapon randomness. The pre-existing own report
    // and quality declaration are explicit referee fixture inputs.
    for e in w.entities.values_mut() {
        e.sensors.clear();
    }
    w.entity_mut(t).unwrap().weapons.clear();
    w.db.weapons.get_mut("us_mk54").unwrap().pk = 0.;
    w.db.weapons.get_mut("us_mk54").unwrap().damage = 0.;
    w.sides[0].contacts.insert(
        ContactId(71),
        harpoon_core::world::Contact {
            id: ContactId(71),
            about: t,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 0.25),
            est_course_deg: 90.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: -1800,
            stale: false,
            sensors: vec!["scenario_assumption:located_sub_report".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules
        .detection
        .as_mut()
        .unwrap()
        .categories
        .insert(format!("0:{}", t.0), harpoon_core::types::PlatformKind::Sub);
    for id in std::iter::once(h).chain(h2) {
        av::apply(
            &mut w,
            id,
            &av::Action::Throttle {
                setting: av::Throttle::Hover,
            },
        )
        .unwrap();
    }
    (w, h, t, h2)
}

fn fire(h: EntityId) -> (Side, Command) {
    (
        Side::Blue,
        Command::Fire {
            unit: h,
            contact: ContactId(71),
            weapon: "us_mk54".into(),
            rounds: 1,
        },
    )
}
fn ammo(w: &World, h: EntityId) -> u16 {
    w.entity(h)
        .unwrap()
        .weapons
        .iter()
        .find(|m| m.weapon == "us_mk54")
        .unwrap()
        .ammo
}
fn restored(w: &World) -> World {
    serde_json::from_slice(&serde_json::to_vec(w).unwrap()).unwrap()
}
fn same_world(a: &World, b: &World) {
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
}
fn released_id(w: &World, h: EntityId) -> EntityId {
    *w.rules
        .torpedoes
        .as_ref()
        .unwrap()
        .launches
        .iter()
        .find(|(_, r)| r.shooter == h)
        .unwrap()
        .0
}
fn placement(w: &World, id: EntityId) -> &Placement {
    w.rules.torpedoes.as_ref().unwrap().placements.placements.get(&format!("10.placement:{}:0",id.0))
        .expect("9.7.1 water entry must roll source placement in the actual Fire tick, without waiting initial_range/2")
}
fn details(w: &World, rule_name: &str) -> Vec<Value> {
    w.events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent { rule, detail, .. } if rule == rule_name => {
                Some(serde_json::from_str(detail).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn receipts(w: &World) -> Vec<Value> {
    harpoon_core::observe::observe(w, Side::Blue, 0, 1000)
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::WeaponsRelease { .. } => Some(serde_json::to_value(&e.kind).unwrap()),
            EventKind::RulesEvent { rule, .. }
                if rule == "10.2.source_launch" || rule == "9.7.1.torpedo_entered_water" =>
            {
                Some(serde_json::to_value(&e.kind).unwrap())
            }
            _ => None,
        })
        .collect()
}
fn raw_results_private(w: &World) {
    for e in &w.events {
        if matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="10.2.4.placement"||rule=="10.2.5.source_terminal")
        {
            assert_eq!(
                e.vis,
                Visibility::Referee,
                "original placement/terminal dice are referee evidence"
            );
        }
    }
    for side in [Side::Blue, Side::Red] {
        let obs = harpoon_core::observe::observe(w, side, 0, 1000);
        assert!(!obs.events.iter().any(|e|matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="10.2.4.placement"||rule=="10.2.5.source_terminal")));
    }
}

/// Select a deterministic seed by raw draws, never force a placement/hit or
/// mutate a projectile. Source random identities are retained by the fix.
fn seed_for(w: &mut World, d10: u8, d100: u8) -> EntityId {
    let id = EntityId(w.next_entity + 1);
    for seed in 1..=200_000 {
        let rng = Rng::new(seed);
        let mut place = rng.derive(Rng::hash_tag(0, 0, &format!("10.placement:{}:0", id.0)));
        let mut hit = rng.derive(Rng::hash_tag(0, id.0, "10.2.5.attack:0"));
        if 1 + (place.next_f64() * 10.) as u8 == d10 && 1 + (hit.next_f64() * 100.) as u8 == d100 {
            w.seed = seed;
            w.rng = rng;
            return id;
        }
    }
    panic!("deterministic raw boundary seed not found");
}

#[test]
fn real_airdrop_d10_nine_and_d100_eighty_place_and_hit_in_the_entry_tick() {
    let (mut w, h, t, _) = fixture("Good", false, false);
    let id = seed_for(&mut w, 9, 80);
    let before = w.entity(t).unwrap().hp;
    let mut copy = restored(&w);
    for world in [&mut w, &mut copy] {
        harpoon_core::sim::tick(world, &[fire(h)]);
    }
    same_world(&w, &copy);
    assert_eq!(released_id(&w, h), id);
    assert_eq!(ammo(&w, h), 3);
    assert_eq!(
        w.rules.aviation.as_ref().unwrap().flights[&h].external_weapon_rounds["us_mk54"],
        3
    );
    let p = placement(&w, id);
    assert_eq!((p.tick, p.d10, p.chance, p.placed), (1, 9, 9, true));
    let f = &w.rules.torpedoes.as_ref().unwrap().flights[&id];
    assert_eq!(
        (f.last_probability, f.last_d100),
        (Some(0.80), Some(80)),
        "Gen4 printed80percent includes80; DB pk0 must not replace the source table"
    );
    assert_eq!(f.run_nm,0.,"entry attack is adjudicated at the real water-entry point before an underwater movement step");
    assert!(
        w.entity(t).is_none_or(|e| e.hp < before),
        "an actual source hit must reach damage, not only a helper probability"
    );
    assert!(w.events.iter().any(|e|matches!(&e.kind,EventKind::Impact{unit,damage,..} if *unit==t&&(*damage-52.).abs()<1e-9)),"US AnnexF Mk5452DP against subs, broad contact aspect; DB damage0 cannot replace it");
    let trace = details(&w, "10.2.5.source_terminal");
    assert_eq!(trace.len(), 1);
    assert_eq!(trace[0]["d100"], 80);
    assert_eq!(trace[0]["hit"], true);
    raw_results_private(&w);
}

#[test]
fn real_airdrop_d100_eighty_one_misses_immediately_and_saves_the_source_reattack() {
    let (mut w, h, t, _) = fixture("Good", false, false);
    let id = seed_for(&mut w, 9, 81);
    let before = w.entity(t).unwrap().hp;
    harpoon_core::sim::tick(&mut w, &[fire(h)]);
    assert_eq!((placement(&w, id).tick, placement(&w, id).d10), (1, 9));
    let f = &w.rules.torpedoes.as_ref().unwrap().flights[&id];
    assert_eq!(
        (f.last_probability, f.last_d100, f.pass),
        (Some(0.80), Some(81), 1)
    );
    assert!(
        matches!(f.phase,torp::Phase::Past{remaining_nm} if (remaining_nm-0.49373650107991357).abs()<1e-12)
    );
    assert_eq!(f.run_nm, 0.);
    assert_eq!(f.fuel, 0.);
    assert_eq!(w.entity(t).unwrap().hp, before);
    assert_eq!(details(&w, "10.2.5.source_terminal")[0]["hit"], false);
    let mut copy = restored(&w);
    for _ in 0..20 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut copy, &[]);
    }
    same_world(&w, &copy);
    assert_eq!(
        details(&w, "10.2.5.source_terminal").len(),
        1,
        "saved first pass cannot reroll each second"
    );
    raw_results_private(&w);
}

#[test]
fn real_airdrop_failed_d10_ten_keeps_one_failed_placement_and_never_attacks_the_designated_target()
{
    let (mut w, h, t, _) = fixture("Good", false, false);
    let id = seed_for(&mut w, 10, 80);
    let before = w.entity(t).unwrap().hp;
    harpoon_core::sim::tick(&mut w, &[fire(h)]);
    let p = placement(&w, id).clone();
    assert_eq!((p.tick, p.d10, p.chance, p.placed), (1, 10, 9, false));
    assert!(matches!(
        w.rules.torpedoes.as_ref().unwrap().flights[&id].phase,
        torp::Phase::Search { .. }
    ));
    assert_eq!(
        w.rules.torpedoes.as_ref().unwrap().flights[&id].last_d100,
        None
    );
    let mut copy = restored(&w);
    for _ in 0..20 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut copy, &[]);
    }
    same_world(&w, &copy);
    assert_eq!(
        serde_json::to_value(placement(&w, id)).unwrap(),
        serde_json::to_value(p).unwrap()
    );
    assert_eq!(w.entity(t).unwrap().hp, before);
    assert!(details(&w, "10.2.5.source_terminal").is_empty());
    raw_results_private(&w);
}

#[test]
fn entry_placement_uses_fair_poor_and_active_anechoic_modifiers_instead_of_contact_quality() {
    for (quality, coating, d10, chance, placed) in [
        ("Fair", false, 6, 6, true),
        ("Poor", false, 3, 3, true),
        ("Good", true, 8, 7, false),
    ] {
        let (mut w, h, _, _) = fixture(quality, coating, false);
        let id = seed_for(&mut w, d10, 80);
        harpoon_core::sim::tick(&mut w, &[fire(h)]);
        let p = placement(&w, id);
        assert_eq!(
            (p.tick, p.d10, p.chance, p.placed),
            (1, d10, chance, placed)
        );
        assert_eq!(
            w.rules.torpedoes.as_ref().unwrap().flights[&id].last_d100,
            if placed { Some(80) } else { None }
        );
        raw_results_private(&w);
    }
}

#[test]
fn both_same_tick_air_drops_register_interference_before_either_immediate_attack() {
    let (mut w, h, t, h2) = fixture("Good", false, true);
    let h2 = h2.unwrap();
    let before = w.entity(t).unwrap().hp;
    let mut copy = restored(&w);
    for world in [&mut w, &mut copy] {
        harpoon_core::sim::tick(world, &[fire(h), fire(h2)]);
    }
    same_world(&w, &copy);
    assert_eq!(w.stats.projectiles_launched, 2);
    assert_eq!(ammo(&w, h), 3);
    assert_eq!(ammo(&w, h2), 3);
    assert_eq!(
        w.rules
            .aircombat
            .as_ref()
            .unwrap()
            .interfered_torpedoes
            .len(),
        2
    );
    assert_eq!(w.entity(t).unwrap().hp, before);
    assert!(
        details(&w, "10.2.5.source_terminal").is_empty(),
        "first callback cannot attack before the second release is known"
    );
    assert!(w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .placements
        .placements
        .is_empty());
    for e in &w.events {
        if matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="9.7.1.torpedo_interference") {
            assert_eq!(e.vis, Visibility::Referee);
        }
    }
}

#[test]
fn hidden_true_domain_depth_and_foil_state_do_not_change_the_release_but_block_an_immediate_hit() {
    let (mut base, h, t, _) = fixture("Good", false, false);
    let id = seed_for(&mut base, 9, 80);
    let mut receipt = None;
    for variant in 0..4 {
        let mut w = base.clone();
        let before = w.entity(t).unwrap().hp;
        match variant {
            1 => {
                let target = w.entity_mut(t).unwrap();
                target.depth_m = 0.;
                target.ordered_depth_m = 0.;
            }
            2 => {
                // A real093 has max_depth300m: movement would correctly clamp
                // a601m mutation back inside Mk54's600m envelope before Fire
                // adjudication. Use an unmistakable author hull for this
                // physical counterfactual; never alter the historical class.
                let class = "scenario_assumption:r054_deep_target";
                let mut hull = w.db.platforms["cn_093"].clone();
                hull.id = class.into();
                hull.name = "Scenario assumed R054 deep target,1000m author limit".into();
                hull.max_depth_m = 1000.;
                w.db.platforms.insert(class.into(), hull);
                let acoustic = w.rules.torpedoes.as_ref().unwrap().config.targets["cn_093"].clone();
                w.rules
                    .torpedoes
                    .as_mut()
                    .unwrap()
                    .config
                    .targets
                    .insert(class.into(), acoustic);
                let target = w.entity_mut(t).unwrap();
                target.class = class.into();
                target.depth_m = 601.;
                target.ordered_depth_m = 601.;
            }
            3 => {
                w.rules
                    .torpedoes
                    .as_mut()
                    .unwrap()
                    .config
                    .targets
                    .get_mut("cn_093")
                    .unwrap()
                    .foil_borne = true
            }
            _ => {}
        }
        let mut copy = restored(&w);
        for world in [&mut w, &mut copy] {
            harpoon_core::sim::tick(world, &[fire(h)]);
        }
        same_world(&w, &copy);
        if variant == 2 {
            assert_eq!(w.entity(t).unwrap().depth_m,601.,"the actual post-movement target must remain beyond source Mk54600m, not be clamped to093's300m");
        }
        assert_eq!(ammo(&w, h), 3);
        assert_eq!(released_id(&w, h), id);
        if let Some(expected) = &receipt {
            assert_eq!(&receipts(&w), expected);
        } else {
            receipt = Some(receipts(&w));
        }
        if variant == 0 {
            assert_eq!(
                w.rules.torpedoes.as_ref().unwrap().flights[&id].last_d100,
                Some(80)
            );
        } else {
            assert_eq!(w.entity(t).unwrap().hp, before);
            assert_eq!(
                w.rules.torpedoes.as_ref().unwrap().flights[&id].last_d100,
                None
            );
        }
        raw_results_private(&w);
    }
}

#[test]
fn missing_actual_target_acoustics_saves_complete_referee_pending_after_an_allowed_release() {
    let (mut w, h, t, _) = fixture("Good", false, false);
    let id = seed_for(&mut w, 9, 80);
    w.entity_mut(t).unwrap().class = "scenario_assumption:unbound_acoustic_target".into();
    let entry = w.entity(h).unwrap().pos;
    harpoon_core::sim::tick(&mut w, &[fire(h)]);
    assert_eq!(ammo(&w, h), 3);
    assert_eq!(released_id(&w, h), id);
    let pending = w
        .rules
        .pending_adjudications
        .get(&id)
        .expect("unknown physical acoustic metadata must remain referee pending");
    assert_eq!(pending.projectile.pos, entry);
    assert_eq!(pending.target, t);
    assert!(w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .placements
        .placements
        .is_empty());
    assert_eq!(
        w.rules.torpedoes.as_ref().unwrap().flights[&id].last_d100,
        None
    );
    let mut copy = restored(&w);
    for _ in 0..3 {
        harpoon_core::sim::tick(&mut w, &[]);
        harpoon_core::sim::tick(&mut copy, &[]);
    }
    same_world(&w, &copy);
    raw_results_private(&w);
    for side in [Side::Blue, Side::Red] {
        assert!(!harpoon_core::observe::observe(&w, side, 0, 1000)
            .events
            .iter()
            .any(|e| matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="10.pending")));
    }
}

#[test]
fn own_release_advice_contains_only_the_source_envelope_and_the_actual_drop_clock() {
    let (mut w, h, _, _) = fixture("Good", false, false);
    seed_for(&mut w, 9, 81);
    let before = harpoon_core::rules::aircombat2026::own_view(&w, h).unwrap();
    assert_eq!(
        before["aerial_torpedo_release"],
        json!({
            "ready_at":0,"remaining_s":0,"max_altitude_m":2000,
            "rotary_allow_hover":true,"max_speed_kts":70.0
        })
    );
    harpoon_core::sim::tick(&mut w, &[fire(h)]);
    let after = harpoon_core::rules::aircombat2026::own_view(&w, h).unwrap();
    assert_eq!(
        after["aerial_torpedo_release"],
        json!({
            "ready_at":181,"remaining_s":180,"max_altitude_m":2000,
            "rotary_allow_hover":true,"max_speed_kts":70.0
        })
    );
    let mut copy = restored(&w);
    harpoon_core::sim::tick(&mut w, &[]);
    harpoon_core::sim::tick(&mut copy, &[]);
    same_world(&w, &copy);
    let view = harpoon_core::rules::aircombat2026::own_view(&w, h).unwrap();
    assert_eq!(view["aerial_torpedo_release"]["remaining_s"], 179);
    assert_eq!(
        view["aerial_torpedo_release"].as_object().unwrap().len(),
        5,
        "advice must not contain target truth, placement, interference, probability or raw dice"
    );
    w.entity_mut(h).unwrap().class = "scenario_assumption:unverified_aircraft".into();
    let unknown = harpoon_core::rules::aircombat2026::own_view(&w, h).unwrap();
    assert!(unknown["aerial_torpedo_release"]["max_speed_kts"].is_null());
}

//! Following-window A1: actual Fire, native failed Placement, then one other
//! coplanar target. The report/Good/environment/signatures are explicit author
//! mechanism inputs; this is not a claim of native airborne source FCS.
//! One-second detection epochs and next-tick acceleration are approved RTS
//! conventions. No existing H1 test or production source is included/changed.
use harpoon_core::rules::{aviation2026 as av, torpedo2026 as torp};
use harpoon_core::{
    Command, ContactId, Database, EntityId, EventKind, Rng, Scenario, Side, Vec2, Visibility, World,
};
use serde_json::{json, Value};
use std::sync::OnceLock;

struct Trial {
    w: World,
    launcher: EntityId,
    original: EntityId,
    opportunity: EntityId,
}

fn fixture(speed_index: usize, outside_range_nm: f64) -> Trial {
    static DB: OnceLock<Database> = OnceLock::new();
    let db = DB
        .get_or_init(|| {
            Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
        })
        .clone();
    let opportunity_y =
        torp::acquisition_nm(4, true, 2, false, false, true).unwrap() + outside_range_nm;
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"a1-single-opportunity","name":"A1 explicit source mechanism exercise","duration_s":2000,"seed":20261024,
        "context":{"rules_profile":"hv2026-rts-v4",
            "aviation_2026":{"source":"US Aircraft2025 PDF69/B64 exact BlockI four-Mk54 carriage; A1 mechanism fixture",
                "loads":{"us_mh60r_mk54x4":{"payload_kg":1140,"tanks":[],"external_weapon_rounds":{"us_mk54":4},
                    "source":"US Aircraft2025 PDF69/B64 and PDF93/E1 noteC"}},"parking":{},"initial_fuel_nm":{"HELO":200}},
            "detection_2026":{"environment":{"source":"scenario_assumption: deep, no layer, A1 physical mechanism water",
                "sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,
                "water_depth_m":1000,"layer":null,"ice":0,"cloud":null}},
            "torpedo_2026":{"source":"scenario_assumption: A1 mechanism inputs, not a real complete sonar/FCS fit",
                "environment":{"source":"scenario_assumption: no layer or reverberation","layer_m":null,"reverberant":false},
                "targets":{"cn_093":{"source":"scenario_assumption: controlled Quiet uncoated targets","size":2,"noise":2,"anechoic":false,"foil_borne":false}},
                "orders":{"HELO:us_mk54":{"source":"scenario_assumption: own declared A1 order",
                    "speed_index":speed_index,"depth_m":50,"enable_run_nm":0,"bearing_deg":0,"spread_deg":[],"cut_wire":false,
                    "solution":{"quality":"Good","source":"scenario_assumption: isolate post-entry capture; not native airborne FCS"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_mh60r_mk54x4","name":"HELO","pos":[0,0],"speed_kts":0,"altitude_m":1000,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"ORIGINAL","pos":[0,0.25],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"OPPORTUNITY","pos":[0,opportunity_y],"course_deg":180,"speed_kts":10,"depth_m":50,"fire_on":false},
            {"side":"Blue","class":"us_ford","name":"scenario_assumption_blue_victory_retainer","pos":[500,500],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"scenario_assumption_red_victory_retainer","pos":[600,600],"speed_kts":0,"depth_m":50,"fire_on":false}]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let find = |name: &str| w.entities.values().find(|e| e.name == name).unwrap().id;
    let launcher = find("HELO");
    let original = find("ORIGINAL");
    let opportunity = find("OPPORTUNITY");
    for e in w.entities.values_mut() {
        e.sensors.clear();
        if e.id != launcher {
            e.weapons.clear();
        }
    }
    // This is explicitly the own located report for ORIGINAL. No contact is
    // inserted for OPPORTUNITY; its acquisition is private physical sensing.
    w.sides[0].contacts.insert(
        ContactId(71),
        harpoon_core::world::Contact {
            id: ContactId(71),
            about: original,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 0.25),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: -1800,
            stale: false,
            sensors: vec!["scenario_assumption: located ORIGINAL report".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", original.0),
        harpoon_core::types::PlatformKind::Sub,
    );
    av::apply(
        &mut w,
        launcher,
        &av::Action::Throttle {
            setting: av::Throttle::Hover,
        },
    )
    .unwrap();
    Trial {
        w,
        launcher,
        original,
        opportunity,
    }
}

fn release(trial: &mut Trial) -> EntityId {
    let id = EntityId(trial.w.next_entity + 1);
    // Select raw native seeds, without writing Placement, its D6s or a terminal
    // die. This boundary requires original D10=10 and the genuine G4 first
    // attack D100<=80 so the actual damage path is exercised deterministically.
    let seed = (1..=200_000)
        .find(|seed| {
            let rng = Rng::new(*seed);
            let mut p = rng.derive(Rng::hash_tag(0, 0, &format!("10.placement:{}:0", id.0)));
            let mut attack = rng.derive(Rng::hash_tag(0, id.0, "10.2.5.attack:0"));
            1 + (p.next_f64() * 10.) as u8 == 10 && 1 + (attack.next_f64() * 100.) as u8 <= 80
        })
        .unwrap();
    trial.w.seed = seed;
    trial.w.rng = Rng::new(seed);
    harpoon_core::sim::tick(
        &mut trial.w,
        &[(
            Side::Blue,
            Command::Fire {
                unit: trial.launcher,
                contact: ContactId(71),
                weapon: "us_mk54".into(),
                rounds: 1,
            },
        )],
    );
    assert_eq!(
        trial.w.stats.projectiles_launched, 1,
        "actual source Fire must release"
    );
    assert!(trial
        .w
        .rules
        .torpedoes
        .as_ref()
        .unwrap()
        .flights
        .contains_key(&id));
    let p = flight(&trial.w, id)
        .placement
        .as_ref()
        .expect("native entry Placement must exist");
    assert_eq!((p.d10, p.placed), (10, false));
    assert_eq!(flight(&trial.w, id).pass, 0);
    assert_eq!(
        trial
            .w
            .entity(trial.launcher)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "us_mk54")
            .unwrap()
            .ammo,
        3
    );
    id
}
fn flight(w: &World, id: EntityId) -> &torp::Flight {
    &w.rules.torpedoes.as_ref().unwrap().flights[&id]
}
fn placements(w: &World) -> Value {
    serde_json::to_value(&w.rules.torpedoes.as_ref().unwrap().placements).unwrap()
}
fn ticks(w: &mut World, count: usize) {
    for _ in 0..count {
        harpoon_core::sim::tick(w, &[]);
    }
}
fn rule_count(w: &World, wanted: &str) -> usize {
    w.events
        .iter()
        .filter(|e| matches!(&e.kind, EventKind::RulesEvent {rule,..} if rule == wanted))
        .count()
}
fn hp(w: &World, id: EntityId) -> f64 {
    w.entity(id).map_or(0., |e| e.hp)
}
fn close(a: f64, b: f64, why: &str) {
    assert!((a - b).abs() < 1e-9, "{why}: {a} != {b}");
}
fn restore(w: &World) -> World {
    serde_json::from_slice(&serde_json::to_vec(w).unwrap()).unwrap()
}
fn assert_captured(w: &World, id: EntityId) {
    assert!(!w.rules.pending_adjudications.contains_key(&id),
        "one source-complete coplanar non-original candidate must actually capture, not remain possible-pending");
    assert!(
        matches!(&flight(w, id).phase, torp::Phase::Locked),
        "physical capture must become homing"
    );
    assert_eq!(
        rule_count(w, "7.5.7.acquired"),
        1,
        "actual acquisition evidence must exist exactly once"
    );
}
fn assert_one_hit(
    trial: &Trial,
    id: EntityId,
    opportunity_hp: f64,
    original_hp: f64,
    dice: &Value,
) {
    assert_eq!(
        rule_count(&trial.w, "10.2.5.source_terminal"),
        1,
        "one genuine first attack must resolve"
    );
    assert!(
        hp(&trial.w, trial.opportunity) < opportunity_hp,
        "actual opportunity target must take source damage"
    );
    close(
        hp(&trial.w, trial.original),
        original_hp,
        "failed original target must not be attacked",
    );
    assert!(
        trial.w.entity(id).is_none_or(|e| !e.alive),
        "successful physical attack retires this torpedo"
    );
    assert_eq!(
        &placements(&trial.w),
        dice,
        "capture/first attack must not reroll Placement or D6"
    );
    assert_eq!(flight(&trial.w, id).pass, 0);
    let ledger =
        serde_json::to_value(&trial.w.rules.torpedoes.as_ref().unwrap().launches[&id]).unwrap();
    assert_eq!(
        ledger["target"],
        json!(trial.original),
        "release history keeps the ORIGINAL target"
    );
    assert_eq!(ledger["contact"], json!(ContactId(71)));
}

#[test]
fn native_failed_entry_acquires_only_the_unique_coplanar_opportunity_and_resolves_one_real_hit() {
    let mut trial = fixture(0, 0.005);
    let old_hp = hp(&trial.w, trial.original);
    let opportunity_hp = hp(&trial.w, trial.opportunity);
    let id = release(&mut trial);
    let dice = placements(&trial.w);
    assert!(
        !trial.w.rules.pending_adjudications.contains_key(&id),
        "entry itself is still clear water"
    );
    let original_position = trial.w.entity(trial.original).unwrap().pos;
    ticks(&mut trial.w, 1);
    let motion = &trial.w.rules.torpedoes.as_ref().unwrap().motion[&trial.opportunity];
    assert_eq!(motion.tick, trial.w.tick);
    assert!(
        motion.from.y > motion.to.y,
        "real native candidate movement must precede detection"
    );
    let source_range = torp::acquisition_nm(4, true, 2, false, false, true).unwrap();
    assert!(
        motion.from.y > source_range && motion.to.y < source_range,
        "native opportunity movement really crosses the source acquisition range in this tick"
    );
    assert_captured(&trial.w, id);
    assert!(
        trial.w.entity(id).unwrap().pos.range_to(original_position) < 0.25,
        "the excluded original really is nearer than the opportunity"
    );
    ticks(&mut trial.w, 200);
    assert_one_hit(&trial, id, opportunity_hp, old_hp, &dice);
}

#[test]
fn slow_search_pays_its_full_capture_tick_then_next_tick_uses_fast_range_without_resetting_fuel() {
    let mut trial = fixture(1, 0.03);
    let id = release(&mut trial);
    let dice = placements(&trial.w);
    ticks(&mut trial.w, 2);
    assert!(!trial.w.rules.pending_adjudications.contains_key(&id));
    close(
        flight(&trial.w, id).fuel,
        2. * (30. / 3600.) / 8.,
        "two native slow radial seconds",
    );
    ticks(&mut trial.w, 1);
    assert_captured(&trial.w, id);
    close(
        trial.w.entity(id).unwrap().speed_kts,
        30.,
        "capture tick retains the old setting",
    );
    close(
        trial.w.entity(id).unwrap().depth_m,
        50.,
        "whole capture tick remains coplanar/radial",
    );
    close(
        flight(&trial.w, id).fuel,
        3. * (30. / 3600.) / 8.,
        "capture tick never retroactively buys fastest motion",
    );
    let fuel = flight(&trial.w, id).fuel;
    let run = flight(&trial.w, id).run_nm;
    ticks(&mut trial.w, 1);
    close(
        trial.w.entity(id).unwrap().speed_kts,
        45.,
        "source automatic maximum setting on next movement",
    );
    close(
        flight(&trial.w, id).fuel,
        fuel + (45. / 3600.) / 6.,
        "retain old fraction, spend one fast second from 6nm",
    );
    close(
        flight(&trial.w, id).run_nm,
        run + 45. / 3600.,
        "actual straight chase path",
    );
    assert_eq!(placements(&trial.w), dice);
}

#[test]
fn complete_world_restore_before_and_after_capture_preserves_the_same_native_first_attack() {
    let mut trial = fixture(0, 0.005);
    let id = release(&mut trial);
    let dice = placements(&trial.w);
    let mut before = restore(&trial.w);
    ticks(&mut trial.w, 1);
    ticks(&mut before, 1);
    assert_captured(&trial.w, id);
    assert_eq!(
        serde_json::to_value(&trial.w).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    let mut after = restore(&trial.w);
    ticks(&mut trial.w, 200);
    ticks(&mut before, 200);
    ticks(&mut after, 200);
    assert_eq!(
        serde_json::to_value(&trial.w).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&trial.w).unwrap(),
        serde_json::to_value(&after).unwrap()
    );
    assert_eq!(rule_count(&trial.w, "10.2.5.source_terminal"), 1);
    assert_eq!(placements(&trial.w), dice);
}

#[test]
fn actual_launcher_takeover_or_removal_does_not_stop_the_released_nonwire_capture_or_attack() {
    let mut trial = fixture(0, 0.005);
    let id = release(&mut trial);
    let dice = placements(&trial.w);
    let mut manual = trial.w.clone();
    let mut removed = trial.w.clone();
    removed.entities.remove(&trial.launcher);
    ticks(&mut trial.w, 1);
    harpoon_core::sim::tick(
        &mut manual,
        &[(
            Side::Blue,
            Command::SetUnitAutomation {
                unit: trial.launcher,
                enabled: false,
            },
        )],
    );
    ticks(&mut removed, 1);
    for w in [&trial.w, &manual, &removed] {
        assert_captured(w, id);
    }
    ticks(&mut trial.w, 200);
    ticks(&mut manual, 200);
    ticks(&mut removed, 200);
    for w in [&manual, &removed] {
        assert_eq!(rule_count(w, "10.2.5.source_terminal"), 1);
        close(
            hp(w, trial.opportunity),
            hp(&trial.w, trial.opportunity),
            "same actual damage after launcher control/lifecycle change",
        );
        assert_eq!(placements(w), dice);
        assert_eq!(
            serde_json::to_value(flight(w, id)).unwrap(),
            serde_json::to_value(flight(&trial.w, id)).unwrap()
        );
    }
}

#[test]
fn actual_capture_and_terminal_evidence_stay_referee_while_own_release_and_ammunition_remain_visible(
) {
    let mut trial = fixture(0, 0.005);
    let id = release(&mut trial);
    ticks(&mut trial.w, 1);
    assert_captured(&trial.w, id);
    ticks(&mut trial.w, 200);
    for rule in ["7.5.7.acquired", "10.2.5.source_terminal"] {
        let events: Vec<_> = trial
            .w
            .events
            .iter()
            .filter(|e| matches!(&e.kind,EventKind::RulesEvent{rule:r,..} if r==rule))
            .collect();
        assert_eq!(
            events.len(),
            1,
            "must inspect a real {rule} event, not an empty set"
        );
        assert_eq!(events[0].vis, Visibility::Referee);
        for side in [Side::Blue, Side::Red] {
            let observed = harpoon_core::observe::observe(&trial.w, side, 0, 1000);
            assert!(!observed
                .events
                .iter()
                .any(|e| matches!(&e.kind,EventKind::RulesEvent{rule:r,..} if r==rule)));
        }
    }
    assert!(
        !trial.w.sides[0]
            .contacts
            .values()
            .any(|c| c.about == trial.opportunity),
        "private seeker creates no free player contact"
    );
    let own = harpoon_core::observe::observe(&trial.w, Side::Blue, 0, 1000);
    assert!(own.events.iter().any(
        |e| matches!(&e.kind,EventKind::WeaponsRelease{shooter,weapon,count,..}
        if *shooter==trial.launcher && weapon==&trial.w.db.weapons["us_mk54"].name && *count==1)
    ));
    assert_eq!(
        trial
            .w
            .entity(trial.launcher)
            .unwrap()
            .weapons
            .iter()
            .find(|m| m.weapon == "us_mk54")
            .unwrap()
            .ammo,
        3
    );
    let view = torp::own_view(&trial.w, Side::Blue, trial.launcher).unwrap();
    assert_eq!(view["releases"][0]["status"], "unknown");
    assert!(view["releases"][0]["physical_alive"].is_null());
}

#[test]
fn an_unknown_acoustic_candidate_preserves_complete_pending_and_never_borrows_default_signature() {
    let mut trial = fixture(0, 0.005);
    let class = "scenario_assumption_unknown_acoustic_candidate";
    let mut hull = trial.w.db.platforms["cn_093"].clone();
    hull.id = class.into();
    hull.name = class.into();
    trial.w.db.platforms.insert(class.into(), hull);
    trial.w.entity_mut(trial.opportunity).unwrap().class = class.into();
    let original_hp = hp(&trial.w, trial.original);
    let other_hp = hp(&trial.w, trial.opportunity);
    let id = release(&mut trial);
    let dice = placements(&trial.w);
    ticks(&mut trial.w, 1);
    let pending = serde_json::to_value(&trial.w.rules.pending_adjudications[&id]).unwrap();
    assert!(pending["projectile"].is_object());
    assert!(pending["evidence"]
        .as_array()
        .is_some_and(|v| !v.is_empty()));
    ticks(&mut trial.w, 20);
    assert_eq!(placements(&trial.w), dice);
    assert_eq!(rule_count(&trial.w, "10.2.5.source_terminal"), 0);
    close(
        hp(&trial.w, trial.original),
        original_hp,
        "unknown candidate cannot retarget the original",
    );
    close(
        hp(&trial.w, trial.opportunity),
        other_hp,
        "unknown signature cannot invent a hit",
    );
}

#[test]
fn multiple_qualified_candidates_keep_the_whole_epoch_pending_without_nearest_or_id_selection() {
    for swap in [false, true] {
        let mut trial = fixture(0, 0.005);
        let mut second = trial.w.entity(trial.opportunity).unwrap().clone();
        second.id = trial.w.allocate_id();
        second.name = "scenario_assumption_second_opportunity".into();
        let second_id = second.id;
        trial.w.entity_mut(trial.opportunity).unwrap().pos.x = if swap { 0.001 } else { -0.001 };
        second.pos.x = if swap { -0.001 } else { 0.001 };
        trial.w.entities.insert(second_id, second);
        let first_hp = hp(&trial.w, trial.opportunity);
        let second_hp = hp(&trial.w, second_id);
        let id = release(&mut trial);
        let dice = placements(&trial.w);
        ticks(&mut trial.w, 1);
        assert!(trial.w.rules.pending_adjudications.contains_key(&id));
        assert_eq!(rule_count(&trial.w, "7.5.7.acquired"), 0);
        ticks(&mut trial.w, 20);
        assert_eq!(rule_count(&trial.w, "10.2.5.source_terminal"), 0);
        assert_eq!(placements(&trial.w), dice);
        close(
            hp(&trial.w, trial.opportunity),
            first_hp,
            "no first-ID attack",
        );
        close(hp(&trial.w, second_id), second_hp, "no second-ID attack");
    }
}

#[test]
fn a_proven_horizontal_cone_exclusion_keeps_real_radial_search_moving_without_a_false_hit_or_hold()
{
    let mut trial = fixture(0, 0.005);
    let e = trial.w.entity_mut(trial.opportunity).unwrap();
    e.pos = Vec2::new(0.5, -0.5);
    e.speed_kts = 0.;
    e.ordered_speed_kts = 0.;
    let other_hp = hp(&trial.w, trial.opportunity);
    let id = release(&mut trial);
    let dice = placements(&trial.w);
    assert!(
        !trial.w.rules.pending_adjudications.contains_key(&id),
        "a source-proven cone exclusion is not an unresolved candidate"
    );
    let before = trial.w.entity(id).unwrap().pos;
    ticks(&mut trial.w, 1);
    assert!(!trial.w.rules.pending_adjudications.contains_key(&id));
    assert_ne!(
        trial.w.entity(id).unwrap().pos,
        before,
        "clear radial segment must actually run"
    );
    assert_eq!(rule_count(&trial.w, "7.5.7.acquired"), 0);
    assert_eq!(rule_count(&trial.w, "10.2.5.source_terminal"), 0);
    close(
        hp(&trial.w, trial.opportunity),
        other_hp,
        "cone-out target cannot be hit",
    );
    assert_eq!(placements(&trial.w), dice);
}

#[test]
fn vertical_unknown_or_surface_domain_mismatch_cannot_copy_enemy_depth_or_create_a_hit() {
    for depth in [100., 0.] {
        let mut trial = fixture(0, 0.005);
        let e = trial.w.entity_mut(trial.opportunity).unwrap();
        e.pos = Vec2::new(0., 0.5);
        e.depth_m = depth;
        e.ordered_depth_m = depth;
        e.speed_kts = 0.;
        e.ordered_speed_kts = 0.;
        let other_hp = hp(&trial.w, trial.opportunity);
        let id = release(&mut trial);
        let dice = placements(&trial.w);
        ticks(&mut trial.w, 1);
        if depth > 50. {
            assert!(
                trial.w.rules.pending_adjudications.contains_key(&id),
                "unbound vertical capture stays adjudicated"
            );
        }
        assert_eq!(rule_count(&trial.w, "10.2.5.source_terminal"), 0);
        close(
            trial.w.entity(id).unwrap().depth_m,
            50.,
            "do not copy an enemy's physical depth",
        );
        close(
            hp(&trial.w, trial.opportunity),
            other_hp,
            "unqualified physical domain/depth cannot take damage",
        );
        assert_eq!(placements(&trial.w), dice);
    }
}

#[test]
fn losing_the_excluded_original_after_real_failed_entry_does_not_veto_the_other_capture() {
    let mut trial = fixture(0, 0.005);
    let opportunity_hp = hp(&trial.w, trial.opportunity);
    let id = release(&mut trial);
    let dice = placements(&trial.w);
    trial.w.entities.remove(&trial.original);
    ticks(&mut trial.w, 1);
    assert_captured(&trial.w, id);
    ticks(&mut trial.w, 200);
    assert_eq!(rule_count(&trial.w, "10.2.5.source_terminal"), 1);
    assert!(hp(&trial.w, trial.opportunity) < opportunity_hp);
    assert_eq!(placements(&trial.w), dice);
    let ledger =
        serde_json::to_value(&trial.w.rules.torpedoes.as_ref().unwrap().launches[&id]).unwrap();
    assert_eq!(ledger["target"], json!(trial.original));
}

fn deep_boundary_fixture() -> (Trial, EntityId) {
    let speed_index = 0;
    let outside_range_nm = 0.005;
    static DB: OnceLock<Database> = OnceLock::new();
    let mut db = DB
        .get_or_init(|| {
            Database::from_json(include_str!("../../../database/consolidated.json")).unwrap()
        })
        .clone();
    // Explicit synthetic test hardware, declared before World construction.
    // This is not a real Annex depth claim and supplies no Motion or Flight.
    let deep_class = "scenario_assumption_deep_competitor";
    let mut hull = db.platforms["cn_093"].clone();
    hull.id = deep_class.into();
    hull.name = deep_class.into();
    hull.max_depth_m = 1200.;
    db.platforms.insert(deep_class.into(), hull);
    let opportunity_y =
        torp::acquisition_nm(4, true, 2, false, false, true).unwrap() + outside_range_nm;
    let mut scenario_json = json!({
        "id":"a1-single-opportunity","name":"A1 explicit source mechanism exercise","duration_s":2000,"seed":20261024,
        "context":{"rules_profile":"hv2026-rts-v4",
            "aviation_2026":{"source":"US Aircraft2025 PDF69/B64 exact BlockI four-Mk54 carriage; A1 mechanism fixture",
                "loads":{"us_mh60r_mk54x4":{"payload_kg":1140,"tanks":[],"external_weapon_rounds":{"us_mk54":4},
                    "source":"US Aircraft2025 PDF69/B64 and PDF93/E1 noteC"}},"parking":{},"initial_fuel_nm":{"HELO":200}},
            "detection_2026":{"environment":{"source":"scenario_assumption: deep, no layer, A1 physical mechanism water",
                "sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,
                "water_depth_m":1000,"layer":null,"ice":0,"cloud":null}},
            "torpedo_2026":{"source":"scenario_assumption: A1 mechanism inputs, not a real complete sonar/FCS fit",
                "environment":{"source":"scenario_assumption: no layer or reverberation","layer_m":null,"reverberant":false},
                "targets":{"cn_093":{"source":"scenario_assumption: controlled Quiet uncoated targets","size":2,"noise":2,"anechoic":false,"foil_borne":false}},
                "orders":{"HELO:us_mk54":{"source":"scenario_assumption: own declared A1 order",
                    "speed_index":speed_index,"depth_m":50,"enable_run_nm":0,"bearing_deg":0,"spread_deg":[],"cut_wire":false,
                    "solution":{"quality":"Good","source":"scenario_assumption: isolate post-entry capture; not native airborne FCS"}}}}},
        "weather":{"visibility_nm":0,"jam":[0,0]},"units":[
            {"side":"Blue","class":"us_mh60r_mk54x4","name":"HELO","pos":[0,0],"speed_kts":0,"altitude_m":1000,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"ORIGINAL","pos":[0,0.25],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"OPPORTUNITY","pos":[0,opportunity_y],"course_deg":180,"speed_kts":10,"depth_m":50,"fire_on":false},
            {"side":"Blue","class":"us_ford","name":"scenario_assumption_blue_victory_retainer","pos":[500,500],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"scenario_assumption_red_victory_retainer","pos":[600,600],"speed_kts":0,"depth_m":50,"fire_on":false}]
    });
    scenario_json["context"]["torpedo_2026"]["targets"][deep_class] = json!({
        "source":"scenario_assumption: known Quiet uncoated competitor; vertical capture remains unsupported",
        "size":2,"noise":2,"anechoic":false,"foil_borne":false
    });
    scenario_json["units"].as_array_mut().unwrap().push(json!({
        "side":"Red","class":deep_class,"name":"DEEP_COMPETITOR",
        "pos":[0.001,opportunity_y],"course_deg":180,"speed_kts":10,"depth_m":599,"fire_on":false
    }));
    let scenario: Scenario = serde_json::from_value(scenario_json).unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let find = |name: &str| w.entities.values().find(|e| e.name == name).unwrap().id;
    let launcher = find("HELO");
    let original = find("ORIGINAL");
    let opportunity = find("OPPORTUNITY");
    let deep = find("DEEP_COMPETITOR");
    for e in w.entities.values_mut() {
        e.sensors.clear();
        if e.id != launcher {
            e.weapons.clear();
        }
    }
    // This is explicitly the own located report for ORIGINAL. No contact is
    // inserted for OPPORTUNITY; its acquisition is private physical sensing.
    w.sides[0].contacts.insert(
        ContactId(71),
        harpoon_core::world::Contact {
            id: ContactId(71),
            about: original,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 0.25),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: 0,
            first_tick: -1800,
            stale: false,
            sensors: vec!["scenario_assumption: located ORIGINAL report".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", original.0),
        harpoon_core::types::PlatformKind::Sub,
    );
    av::apply(
        &mut w,
        launcher,
        &av::Action::Throttle {
            setting: av::Throttle::Hover,
        },
    )
    .unwrap();
    (
        Trial {
            w,
            launcher,
            original,
            opportunity,
        },
        deep,
    )
}

// An endpoint below the weapon limit does not prove that this entire epoch
// was unreachable. Only native SetDepth/movement may create this depth path.
#[test]
fn a_native_depth_crossing_beyond_weapon_maximum_cannot_hide_an_unknown_competitor_and_select_another(
) {
    let (mut trial, deep) = deep_boundary_fixture();
    close(
        trial.w.entity(deep).unwrap().depth_m,
        599.,
        "author initial depth",
    );
    let stable_hp = hp(&trial.w, trial.opportunity);
    let deep_hp = hp(&trial.w, deep);
    let id = release(&mut trial);
    let dice = placements(&trial.w);
    assert!(
        !trial.w.rules.pending_adjudications.contains_key(&id),
        "both competitors remain outside the source envelope during the actual failed entry"
    );
    close(
        trial.w.entity(deep).unwrap().depth_m,
        599.,
        "entry retains declared depth",
    );
    close(
        flight(&trial.w, id).weapon.max_depth_m,
        600.,
        "actual Mk54 source limit",
    );
    harpoon_core::sim::tick(
        &mut trial.w,
        &[(
            Side::Red,
            Command::SetDepth {
                unit: deep,
                depth_m: 602.,
            },
        )],
    );
    let state = trial.w.rules.torpedoes.as_ref().unwrap();
    let depth = &state.depth_motion[&deep];
    assert_eq!(depth.tick, trial.w.tick);
    assert_eq!(depth.from, Some(599.));
    close(
        depth.to,
        602.,
        "native three-metre vertical movement crosses the actual source limit",
    );
    close(
        trial.w.entity(deep).unwrap().depth_m,
        602.,
        "actual physical end depth",
    );
    let source_range = torp::acquisition_nm(4, true, 2, false, false, true).unwrap();
    let motion = &state.motion[&deep];
    assert_eq!(motion.tick, trial.w.tick);
    assert!(
        motion.from.y > source_range && motion.to.y < source_range,
        "native horizontal Motion enters the local acoustic volume during this same epoch"
    );
    let at_max = (600. - depth.from.unwrap()) / (depth.to - depth.from.unwrap());
    let candidate_at_max = motion.from.add(motion.to.sub(motion.from).scaled(at_max));
    let old_setting_travel = flight(&trial.w, id).weapon.settings[0].speed_kts / 3600.;
    let torpedo_at_max = Vec2::zero().advanced(0., old_setting_travel * at_max);
    assert!(
        torpedo_at_max.range_to(candidate_at_max) < source_range,
        "the actual relative sweep already enters range before the native depth path leaves600m"
    );
    let stable = &state.depth_motion[&trial.opportunity];
    assert_eq!((stable.from, stable.to), (Some(50.), 50.));
    let stable_motion = &state.motion[&trial.opportunity];
    assert!(stable_motion.from.y > source_range && stable_motion.to.y < source_range,
        "a separate stable candidate really is eligible, so endpoint-skipping the deep competitor would choose it");
    assert!(trial.w.rules.pending_adjudications.contains_key(&id),
        "599-to602m is not proof that the complete epoch stayed beyond600m; retain the unknown vertical competitor");
    assert_eq!(rule_count(&trial.w, "7.5.7.acquired"), 0);
    let pending = serde_json::to_value(&trial.w.rules.pending_adjudications[&id]).unwrap();
    assert!(pending["projectile"].is_object());
    assert!(pending["evidence"]
        .as_array()
        .is_some_and(|events| !events.is_empty()));
    ticks(&mut trial.w, 20);
    assert_eq!(placements(&trial.w), dice);
    assert_eq!(flight(&trial.w, id).pass, 0);
    assert_eq!(rule_count(&trial.w, "10.2.5.source_terminal"), 0);
    close(
        hp(&trial.w, trial.opportunity),
        stable_hp,
        "no silent stable-candidate selection",
    );
    close(
        hp(&trial.w, deep),
        deep_hp,
        "no unsupported vertical attack",
    );
}

// DRAFT ONLY: append this helper and test to torpedo_initial_acquisition_2026.rs
// only after root authorizes registration. Not part of a crate, not compiled.
// Reuses its existing Trial/release/ticks/flight/placements/hp/close/rule_count.
// All bodies are existing real cn_093/us_mh60r_mk54x4/us_ford classes. The
// navigation geometry, located report, Good solution and acoustic fit are
// explicitly author assumptions, not a complete real hardware assertion.
fn native_surface_domain_fixture_case13() -> (Trial, EntityId) {
    let db = Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
    let source_range = torp::acquisition_nm(4, true, 2, false, false, true).unwrap();
    // After 178 real prelaunch ticks, both moving candidates are at R+.005.
    // Entry tick179 leaves them outside R; tick180 carries both into R while
    // only SURFACING completes the actual source Surface180s operation.
    let initial_y = source_range + 0.005 + 178. * 10. / 3600.;
    let maximum = db.platforms["cn_093"].max_depth_m;
    let submerged_speed = db.platforms["cn_093"].max_speed_kts;
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"a1-source-surface-domain-epoch", "name":"A1 actual Surface completion competitor",
        "duration_s":2000,"seed":20261024,
        "context":{"rules_profile":"hv2026-rts-v4",
            "radar_environment":{"sea_state":0,"precipitation":"None"},
            "navigation_2026":{
                "source":"scenario_assumption: controlled Surface180s geometry, no claimed real cn093 navigation fit",
                "units":{"SURFACING":{
                    "source":"scenario_assumption: source Surface mechanism with explicit author water and navigation inputs",
                    "size":"C","merchant":false,"passenger_liner":false,"cpp":false,"waterjet":false,
                    "undamaged_max_kts":submerged_speed,"propulsion":"NonSteam","readiness":"Underway",
                    "submarine":{"source":"scenario_assumption: explicit author depth/surface/water fit for Surface clock",
                        "maximum_depth_m":maximum,"surface_max_kts":12,"water_depth_m":1000,"very_small":false}
                }}
            },
            "aviation_2026":{"source":"US Aircraft2025 PDF69/B64 exact BlockI four-Mk54 carriage; A1 mechanism fixture",
                "loads":{"us_mh60r_mk54x4":{"payload_kg":1140,"tanks":[],"external_weapon_rounds":{"us_mk54":4},
                    "source":"US Aircraft2025 PDF69/B64 and PDF93/E1 noteC"}},"parking":{},"initial_fuel_nm":{"HELO":200}},
            "detection_2026":{"environment":{"source":"scenario_assumption: deep no-layer A1 mechanism water",
                "sea_state":0,"precipitation":0,"visibility_percent":100,"shipping":0,"biologics":0,
                "water_depth_m":1000,"layer":null,"ice":0,"cloud":null}},
            "torpedo_2026":{"source":"scenario_assumption: A1 physical inputs, not native airborne FCS",
                "environment":{"source":"scenario_assumption: no layer or reverberation","layer_m":null,"reverberant":false},
                "targets":{"cn_093":{"source":"scenario_assumption: known Quiet, uncoated, nonfoil targets",
                    "size":2,"noise":2,"anechoic":false,"foil_borne":false}},
                "orders":{"HELO:us_mk54":{"source":"scenario_assumption: own A1 order",
                    "speed_index":0,"depth_m":50,"enable_run_nm":0,"bearing_deg":0,"spread_deg":[],"cut_wire":false,
                    "solution":{"quality":"Good","source":"scenario_assumption: isolate failed entry and capture; not native aircraft FCS"}}}}
        },
        "weather":{"visibility_nm":0,"jam":[0,0]},
        "units":[
            {"side":"Blue","class":"us_mh60r_mk54x4","name":"HELO","pos":[0,0],"speed_kts":0,"altitude_m":1000,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"ORIGINAL","pos":[0,0.25],"course_deg":0,"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"OPPORTUNITY","pos":[0,initial_y],"course_deg":180,"speed_kts":10,"depth_m":50,"fire_on":false},
            {"side":"Blue","class":"us_ford","name":"scenario_assumption_blue_victory_retainer","pos":[500,500],"speed_kts":0,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"scenario_assumption_red_victory_retainer","pos":[600,600],"speed_kts":0,"depth_m":50,"fire_on":false},
            {"side":"Red","class":"cn_093","name":"SURFACING","pos":[0.001,initial_y],"course_deg":180,"speed_kts":10,"depth_m":50,"fire_on":false}
        ]
    })).unwrap();
    let mut w = harpoon_core::load_scenario(scenario, db).unwrap();
    let find = |name: &str| w.entities.values().find(|e| e.name == name).unwrap().id;
    let launcher = find("HELO");
    let original = find("ORIGINAL");
    let opportunity = find("OPPORTUNITY");
    let surfacing = find("SURFACING");
    for e in w.entities.values_mut() {
        e.sensors.clear();
        if e.id != launcher {
            e.weapons.clear();
        }
    }
    av::apply(
        &mut w,
        launcher,
        &av::Action::Throttle {
            setting: av::Throttle::Hover,
        },
    )
    .unwrap();
    assert!(harpoon_core::rules::navigation2026::submarine::configured_depth(&w, surfacing));
    harpoon_core::sim::tick(
        &mut w,
        &[(
            Side::Red,
            Command::SetDepth {
                unit: surfacing,
                depth_m: 0.,
            },
        )],
    );
    ticks(&mut w, 177);
    assert_eq!(w.tick, 178);
    close(
        w.entity(surfacing).unwrap().depth_m,
        50.,
        "source Surface is still incomplete",
    );
    let op = w.rules.navigation.as_ref().unwrap().units[&surfacing]
        .depth_operation
        .as_ref()
        .unwrap();
    assert_eq!(serde_json::to_value(op).unwrap()["kind"], "Surface");
    close(
        op.worked_seconds,
        178.,
        "178 actual source movement seconds, no edited progress",
    );

    // Explicit fresh author knowledge about ORIGINAL, after the prelaunch
    // clock. There is no contact for either seeker opportunity. This is the
    // same declared-report assumption as the existing A1 native tests.
    w.sides[0].contacts.insert(
        ContactId(71),
        harpoon_core::world::Contact {
            id: ContactId(71),
            about: original,
            category: harpoon_core::types::PlatformKind::Sub,
            class_name: None,
            estimate: Vec2::new(0., 0.25),
            est_course_deg: 0.,
            est_speed_kts: 0.,
            bearing_only: false,
            quality: 1.,
            last_update_tick: w.tick,
            first_tick: w.tick - 1800,
            stale: false,
            sensors: vec!["scenario_assumption: fresh located ORIGINAL report".into()],
            offset: Vec2::zero(),
            announced: true,
        },
    );
    w.rules.detection.as_mut().unwrap().categories.insert(
        format!("0:{}", original.0),
        harpoon_core::types::PlatformKind::Sub,
    );
    (
        Trial {
            w,
            launcher,
            original,
            opportunity,
        },
        surfacing,
    )
}

#[test]
fn native_source_surface_completion_cannot_hide_a_same_epoch_submarine_competitor_and_select_another(
) {
    let (mut trial, surfacing) = native_surface_domain_fixture_case13();
    let stable_hp = hp(&trial.w, trial.opportunity);
    let surfacing_hp = hp(&trial.w, surfacing);
    let original_hp = hp(&trial.w, trial.original);
    let id = release(&mut trial); // unchanged native seed-selection helper
    let seed = trial.w.seed;
    let dice = placements(&trial.w);
    assert_eq!(trial.w.tick, 179);
    assert!(
        !trial.w.rules.pending_adjudications.contains_key(&id),
        "actual entry is outside both candidates"
    );
    close(
        trial.w.entity(surfacing).unwrap().depth_m,
        50.,
        "source Surface has not completed at179",
    );
    let op = trial.w.rules.navigation.as_ref().unwrap().units[&surfacing]
        .depth_operation
        .as_ref()
        .unwrap();
    assert_eq!(serde_json::to_value(op).unwrap()["kind"], "Surface");
    close(
        op.worked_seconds,
        179.,
        "native source Surface179 checkpoint",
    );
    let body_before = trial.w.entity(id).unwrap().pos;
    let heading = flight(&trial.w, id).helical.as_ref().unwrap().heading_deg;
    let old_travel = flight(&trial.w, id).weapon.settings[0].speed_kts / 3600.;
    assert!(!flight(&trial.w, id).weapon.surface && flight(&trial.w, id).weapon.submarine);

    ticks(&mut trial.w, 1);
    assert_eq!(trial.w.tick, 180);
    let body = trial.w.entity(surfacing).unwrap();
    assert_eq!(body.class, "cn_093");
    assert_eq!(body.kind, harpoon_core::types::PlatformKind::Sub);
    assert_eq!(body.combat_kind(), harpoon_core::types::PlatformKind::Ship);
    close(
        body.depth_m,
        0.,
        "only the actual180-second Surface completion changed depth/domain",
    );
    assert!(trial.w.rules.navigation.as_ref().unwrap().units[&surfacing]
        .depth_operation
        .is_none());
    let state = trial.w.rules.torpedoes.as_ref().unwrap();
    let depth = &state.depth_motion[&surfacing];
    assert_eq!(depth.tick, trial.w.tick);
    assert_eq!(depth.from, Some(50.));
    close(
        depth.to,
        0.,
        "native source DepthMotion50-to0, not direct test mutation",
    );
    let stable_depth = &state.depth_motion[&trial.opportunity];
    assert_eq!((stable_depth.from, stable_depth.to), (Some(50.), 50.));
    let range = torp::acquisition_nm(4, true, 2, false, false, true).unwrap();
    for target in [surfacing, trial.opportunity] {
        let motion = &state.motion[&target];
        assert_eq!(motion.tick, trial.w.tick);
        assert!(
            motion.from.y > range && motion.to.y < range,
            "each actual native horizontal path enters the source range in this epoch"
        );
        let halfway = motion.from.add(motion.to.sub(motion.from).scaled(0.5));
        let weapon_halfway = body_before.advanced(heading, old_travel * 0.5);
        assert!(
            weapon_halfway.range_to(halfway) < range,
            "the source relative radial sweep is already inside range before the epoch end"
        );
        assert!(
            harpoon_core::geom::angle_diff(heading, weapon_halfway.bearing_to(halfway)).abs() < 60.
        );
    }

    // FAILS on the frozen081d... implementation: its endpoint combat_kind
    // domain filter skips SURFACING before considering its real depth history,
    // then incorrectly locks the other candidate as unique.
    assert!(trial.w.rules.pending_adjudications.contains_key(&id),
        "end-domainShip is not proof the entire epoch was outside the Sub seeker domain; keep the whole decision pending");
    assert_eq!(rule_count(&trial.w, "7.5.7.acquired"), 0);
    assert_eq!(rule_count(&trial.w, "10.2.5.source_terminal"), 0);
    assert_eq!(placements(&trial.w), dice);
    assert_eq!(flight(&trial.w, id).pass, 0);
    let pending = serde_json::to_value(&trial.w.rules.pending_adjudications[&id]).unwrap();
    assert!(pending["projectile"].is_object());
    assert!(pending["evidence"]
        .as_array()
        .is_some_and(|events| !events.is_empty()));
    ticks(&mut trial.w, 20);
    assert_eq!(trial.w.seed, seed);
    assert_eq!(placements(&trial.w), dice);
    assert_eq!(flight(&trial.w, id).pass, 0);
    assert_eq!(rule_count(&trial.w, "7.5.7.acquired"), 0);
    assert_eq!(rule_count(&trial.w, "10.2.5.source_terminal"), 0);
    close(
        hp(&trial.w, trial.opportunity),
        stable_hp,
        "no stable-target selection/attack",
    );
    close(
        hp(&trial.w, surfacing),
        surfacing_hp,
        "no unsupported vertical/domain attack",
    );
    close(
        hp(&trial.w, trial.original),
        original_hp,
        "failed original stays excluded",
    );
}

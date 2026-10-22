//! Source-derived acceptance, as opposed to tests of the legacy approximation.
use harpoon_core::{
    air, damage, db::Database, load_scenario, AirMission, EventKind, Scenario, Vec2, World,
};
use serde_json::json;

fn fixture() -> World {
    let mut db = Database::built_in();
    for name in ["cn_navy", "us_navy"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../database/volumes/{name}.json"));
        db = db.merged(&Database::from_json(&std::fs::read_to_string(path).unwrap()).unwrap());
    }
    let sc: Scenario = serde_json::from_value(
        json!({"id":"source-acceptance","name":"Source acceptance","seed":20261021,
        "duration_s":100000,"weather":{"visibility_nm":12,"jam":[0,0],"overcast":0,"rain":0},
        "units":[{"side":"Blue","class":"us_ford","name":"Carrier","pos":{"x":0,"y":0}}]}),
    )
    .unwrap();
    load_scenario(sc, db).unwrap()
}

#[test]
fn damage_speed_uses_annex_threshold() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    w.entity_mut(id).unwrap().hp = 833.0;
    assert_eq!(
        w.entity(id).unwrap().max_speed(&w.db),
        16.0,
        "HV14.1.1/America A4: 834 DP damage means max16kt"
    );
}

#[test]
fn massive_damage_prevents_carrier_launch() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    w.entity_mut(id).unwrap().hp = 1667.0 / 4.0;
    assert!(
        air::launch_aircraft(
            &mut w,
            id,
            "fighter",
            1,
            &AirMission::Recon {
                point: Vec2::new(0.0, 10.0)
            }
        )
        .is_err(),
        "HV14.2.1: only25% DP remaining"
    );
}

#[test]
fn legacy_flight_deck_marker_cannot_be_silently_ignored() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    w.entity_mut(id)
        .unwrap()
        .ext
        .insert("flight_deck_hit".into(), 1.0);
    assert!(air::launch_aircraft(
        &mut w,
        id,
        "fighter",
        1,
        &AirMission::Recon {
            point: Vec2::new(0.0, 10.0)
        }
    )
    .is_err());
}

#[test]
fn yu6_uses_surface_dp_and_influence_fuze() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    let torp = w.db.weapons["cn_yu6"].clone();
    damage::apply_hit_at(&mut w, id, &torp, true, Some(90.0));
    let d = w
        .events
        .iter()
        .find_map(|e| match e.kind {
            EventKind::Impact { damage, .. } => Some(damage),
            _ => None,
        })
        .unwrap();
    assert_eq!(d, 141.0, "China F1: ship141, sub100, influence ignores TPS");
}

#[test]
fn prh_airburst_has_no_primary_hull_dp() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    // Test the published airburst procedure on an unarmored target. Ford's
    // armored deck is a separate protection case, not an unarmored control.
    w.entity_mut(id)
        .unwrap()
        .rule_data
        .as_mut()
        .unwrap()
        .deck_cm = 0.0;
    let arm = w.db.weapons["cn_yj91"].clone();
    damage::apply_hit_at(&mut w, id, &arm, false, Some(90.0));
    for e in &w.events {
        if let EventKind::Impact { damage, .. } = e.kind {
            assert_eq!(damage, 0.0, "HV7.4.2/14.1.4: PRH airburst");
        }
    }
    assert!(w
        .events
        .iter()
        .any(|e| matches!(&e.kind,EventKind::CriticalHit{system,..} if system=="radar")));
}

#[test]
fn small_damage_never_bypasses_one_percent_critical_gate() {
    let base = fixture();
    let id = *base.entities.keys().next().unwrap();
    let gun = base.db.weapons["gun_30mm"].clone();
    assert!(gun.damage < 16.67);
    for seed in 0..256 {
        let mut w = base.clone();
        w.rng = harpoon_core::Rng::new(seed);
        w.seed = seed;
        damage::apply_hit_at(&mut w, id, &gun, false, Some(90.0));
        assert!(
            !w.events
                .iter()
                .any(|e| matches!(e.kind, EventKind::CriticalHit { .. })),
            "HV14.1.2/14.1.3: below1% in isolated tick; seed={seed}"
        );
    }
}

#[test]
fn new_scenarios_version_the_rules_and_old_worlds_keep_legacy() {
    let w = fixture();
    assert_eq!(
        w.rules.profile,
        harpoon_core::rules::damage_runtime::RuleProfile::Rts2026
    );
    let mut raw = serde_json::to_value(&w).unwrap();
    raw.as_object_mut().unwrap().remove("rules");
    // A historical World has no new per-entity rule marker either.
    for e in raw["entities"].as_object_mut().unwrap().values_mut() {
        e["ext"].as_object_mut().unwrap().remove("rules.v2");
    }
    let old: World = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(
        old.rules.profile,
        harpoon_core::rules::damage_runtime::RuleProfile::Legacy
    );
    assert_eq!(
        serde_json::to_value(old).unwrap(),
        raw,
        "missing legacy fields must not silently rewrite saved state"
    );
}

#[test]
fn same_tick_hits_have_distinct_auditable_random_identities() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    let m = w.db.weapons["cn_yj18"].clone();
    for _ in 0..2 {
        damage::apply_hit_at(&mut w, id, &m, false, Some(90.0));
    }
    let hits: std::collections::BTreeSet<_> = w
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::RulesEvent {
                domain,
                rule,
                detail,
                ..
            } if domain == "damage" && rule == "damage_roll" => {
                let v: serde_json::Value = serde_json::from_str(detail).unwrap();
                v["hit"].as_str().map(str::to_owned)
            }
            _ => None,
        })
        .collect();
    assert!(
        hits.iter().any(|s| s.contains("manual:0:1:1"))
            && hits.iter().any(|s| s.contains("manual:0:1:2"))
    );
}

#[test]
fn tick_hit_batch_is_order_independent_and_survives_save_before_resolution() {
    use harpoon_core::rules::damage_runtime as rt;
    let mut a = fixture();
    let mut b = a.clone();
    let id = *a.entities.keys().next().unwrap();
    let x = a.db.weapons["cn_yj18"].clone();
    let y = a.db.weapons["cn_yj83"].clone();
    rt::begin(&mut a);
    rt::begin(&mut b);
    for (w, reverse) in [(&mut a, false), (&mut b, true)] {
        let shots = if reverse {
            vec![(&y, "shot-y"), (&x, "shot-x")]
        } else {
            vec![(&x, "shot-x"), (&y, "shot-y")]
        };
        for (weapon, key) in shots {
            rt::queue(w, id, weapon, false, Some(90.0), None, Some(key.into()));
        }
        assert_eq!(
            w.entity(id).unwrap().hp,
            1667.0,
            "damage must wait until all eligible tick actions are collected"
        );
    }
    let mut b: World = serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap();
    rt::finish(&mut a);
    rt::finish(&mut b);
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
}

#[test]
fn a_replayed_hit_identity_is_not_applied_twice() {
    use harpoon_core::rules::damage_runtime as rt;
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    let m = w.db.weapons["cn_yj18"].clone();
    rt::queue(
        &mut w,
        id,
        &m,
        false,
        Some(90.0),
        None,
        Some("projectile-100".into()),
    );
    let after = serde_json::to_value(&w).unwrap();
    rt::queue(
        &mut w,
        id,
        &m,
        false,
        Some(90.0),
        None,
        Some("projectile-100".into()),
    );
    assert_eq!(after, serde_json::to_value(w).unwrap());
}

#[test]
fn carrier_uses_aviation_column_and_independent_automatic_fire() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    let m = w.db.weapons["cn_yj18"].clone();
    damage::apply_hit_at(&mut w, id, &m, false, Some(90.0));
    assert!(w.events.iter().any(|e| match &e.kind {
        EventKind::RulesEvent { rule, detail, .. } if rule == "hit_resolved" => {
            serde_json::from_str::<serde_json::Value>(detail).unwrap()["column"] == "aviation"
        }
        _ => false,
    }));
    assert!(w.events.iter().any(|e| match &e.kind {
        EventKind::RulesEvent { rule, detail, .. } if rule == "damage_roll" => {
            serde_json::from_str::<serde_json::Value>(detail).unwrap()["hit"]
                .as_str()
                .is_some_and(|s| s.contains("automatic_fire"))
        }
        _ => false,
    }));
}

#[test]
fn ordinary_fire_does_not_drain_hull_during_first_seconds() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    let m = w.db.weapons["cn_yj18"].clone();
    damage::apply_hit_at(&mut w, id, &m, false, Some(90.0));
    let hp = w.entity(id).unwrap().hp;
    assert_eq!(
        hp,
        1667.0 - 33.0,
        "dice following annex DP produce extra criticals, not hull jitter"
    );
    for _ in 0..10 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(w.entity(id).unwrap().hp, hp);
}

#[test]
fn no_automatic_sensor_repair_at_five_minutes() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    w.entity_mut(id).unwrap().systems.radar = 0.0;
    for _ in 0..360 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(
        w.entity(id).unwrap().systems.radar,
        0.0,
        "14.6 repair is post-battle, not300 quiet seconds"
    );
}

#[test]
fn hit_rolls_are_private_to_the_victims_side() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    let m = w.db.weapons["cn_yj18"].clone();
    damage::apply_hit_at(&mut w, id, &m, false, Some(90.0));
    assert!(w
        .events_for(harpoon_core::Side::Red, 0)
        .iter()
        .all(|e| !matches!(&e.kind,EventKind::RulesEvent{domain,..}if domain=="damage")));
}

#[test]
fn bridge_casualty_holds_motion_and_delays_new_orders() {
    use harpoon_core::{sim, Command, Side};
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    {
        let e = w.entity_mut(id).unwrap();
        e.course_deg = 0.0;
        e.speed_kts = 10.0;
        e.ordered_speed_kts = 10.0;
        e.ext.insert("damage.bridge_until".into(), 3.0);
        e.ext.insert("damage.bridge_order_delay".into(), 360.0);
        e.ext.insert("damage.bridge_course".into(), 0.0);
        e.ext.insert("damage.bridge_speed".into(), 10.0);
    }
    sim::tick(
        &mut w,
        &[
            (
                Side::Blue,
                Command::SetCourse {
                    unit: id,
                    course_deg: 90.0,
                },
            ),
            (
                Side::Blue,
                Command::SetSpeed {
                    unit: id,
                    speed_kts: 20.0,
                },
            ),
        ],
    );
    assert_eq!(
        w.entity(id).unwrap().course_deg,
        0.0,
        "14.5 Bridge holds current course"
    );
    assert_eq!(w.entity(id).unwrap().speed_kts, 10.0);
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for _ in 1..362 {
        sim::tick(&mut w, &[]);
        sim::tick(&mut restored, &[]);
    }
    assert_eq!(
        w.entity(id).unwrap().course_deg,
        0.0,
        "correcting the casualty does not remove the two-TT order delay"
    );
    sim::tick(&mut w, &[]);
    sim::tick(&mut restored, &[]);
    assert!(w.entity(id).unwrap().course_deg > 0.0 && w.entity(id).unwrap().speed_kts > 10.0);
    assert_eq!(
        serde_json::to_value(w).unwrap(),
        serde_json::to_value(restored).unwrap()
    );
}

#[test]
fn jammed_rudder_ignores_a_new_course_and_disabled_rudder_slows_turning() {
    use harpoon_core::{sim, Command, Side};
    let mut jam = fixture();
    let id = *jam.entities.keys().next().unwrap();
    jam.entity_mut(id)
        .unwrap()
        .ext
        .insert("damage.rudder_jammed".into(), 1.0);
    sim::tick(
        &mut jam,
        &[(
            Side::Blue,
            Command::SetCourse {
                unit: id,
                course_deg: 90.0,
            },
        )],
    );
    assert_eq!(
        jam.entity(id).unwrap().course_deg,
        0.0,
        "14.5 jammed straight rudder"
    );
    let mut normal = fixture();
    let mut disabled = normal.clone();
    disabled
        .entity_mut(id)
        .unwrap()
        .ext
        .insert("damage.rudder_disabled".into(), 1.0);
    let order = [(
        Side::Blue,
        Command::SetCourse {
            unit: id,
            course_deg: 90.0,
        },
    )];
    sim::tick(&mut normal, &order);
    sim::tick(&mut disabled, &order);
    assert!(
        (disabled.entity(id).unwrap().course_deg * 3.0 - normal.entity(id).unwrap().course_deg)
            .abs()
            < 1e-9,
        "RTS 45-to-15 degree advance limit maps to one-third angular rate"
    );
}

#[test]
fn a_massively_damaged_submarine_can_reach_the_surface() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    {
        let e = w.entity_mut(id).unwrap();
        e.kind = harpoon_core::types::PlatformKind::Sub;
        e.depth_m = 30.0;
        e.ordered_depth_m = 30.0;
        e.hp = e.hp_max * 0.25;
    }
    for _ in 0..240 {
        harpoon_core::sim::tick(&mut w, &[]);
    }
    assert_eq!(
        w.entity(id).unwrap().depth_m,
        0.0,
        "14.2.1 must surface; depth10 clamp cannot prevent this"
    );
}

#[test]
fn emergency_magazine_flooding_is_an_owned_persistent_command() {
    use harpoon_core::{sim, Command, Side};
    let command: Command =
        serde_json::from_value(json!({"cmd":"damage_control","unit":1,"action":"flood_magazines"}))
            .expect("14.4/14.5 must be available to commanders");
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    sim::tick(&mut w, &[(Side::Red, command.clone())]);
    assert!(
        w.entity(id).unwrap().weapons.iter().any(|m| m.ammo > 0),
        "enemy must not flood our magazines"
    );
    sim::tick(&mut w, &[(Side::Blue, command)]);
    assert!(w.entity(id).unwrap().weapons.iter().all(|m| m.ammo == 0));
    assert_eq!(
        w.entity(id).unwrap().ext.get("damage.magazines_flooded"),
        Some(&1.0)
    );
    let restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(w).unwrap(),
        serde_json::to_value(restored).unwrap()
    );
}

#[test]
fn anti_radiation_launch_requires_an_emitting_radar() {
    let mut w = fixture();
    let shooter = *w.entities.keys().next().unwrap();
    let mut target = w.entity(shooter).unwrap().clone();
    target.id = w.allocate_id();
    target.side = harpoon_core::Side::Red;
    target.ext.insert("emission.radar".into(), 0.0);
    target.pos = Vec2::new(0.0, 20.0);
    let target_id = target.id;
    w.entities.insert(target_id, target);
    let def = w.db.weapons["cn_yj91"].clone();
    assert!(
        harpoon_core::weapons::launch(&mut w, shooter, &def, Some(target_id), None).is_none(),
        "7.4.2 PRH needs a radiating target, not the generic active-radar ASM path"
    );
    w.entity_mut(target_id)
        .unwrap()
        .ext
        .insert("emission.radar".into(), 1.0);
    assert!(harpoon_core::weapons::launch(&mut w, shooter, &def, Some(target_id), None).is_some());
}

#[test]
fn annex_variants_and_notation_are_not_inferred_from_damage_bands() {
    let w = fixture();
    assert_eq!(w.db.weapons["cn_yj12"].range_nm, 162.0);
    assert_eq!(w.db.weapons["cn_yj12_surface_skim"].range_nm, 54.0);
    assert_eq!(w.db.weapons["cn_yj12_air"].range_nm, 216.0);
    assert_eq!(w.db.weapons["cn_yj12_air_skim"].range_nm, 110.0);
    assert_eq!(
        w.db.weapons["cn_yj12"]
            .rule_data
            .as_ref()
            .unwrap()
            .penetration_cm,
        Some(13.0)
    );
    assert_eq!(
        w.db.weapons["cn_yj12_air"]
            .rule_data
            .as_ref()
            .unwrap()
            .penetration_cm,
        Some(9.0)
    );
    assert_eq!(
        w.db.weapons["cn_yj62"].rule_data.as_ref().unwrap().extra,
        "D6",
        "China D1 prints44+D6, not the generic25-44DP table's D6/2"
    );
    assert_eq!(
        w.db.weapons["us_mk48"].rule_data.as_ref().unwrap().sub_dp,
        Some(124.0)
    );
    assert_eq!(
        w.db.weapons["us_mk54"].range_nm, 6.0,
        "F2:45kt setting is6nm;8nm belongs to30kt"
    );
    assert_eq!(
        w.db.weapons["cn_yj9"]
            .rule_data
            .as_ref()
            .unwrap()
            .min_launch_altitude_m,
        21.0,
        "rail-launched exception is legal in VLow"
    );
}

#[test]
fn opposite_sides_network_interleaving_cannot_change_tick_outcome() {
    use harpoon_core::{sim, Command, EntityId, Side};
    let mut a = fixture();
    let mut target = a.entity(EntityId(1)).unwrap().clone();
    target.id = a.allocate_id();
    target.side = Side::Red;
    target.pos = Vec2::new(0.0, 20.0);
    a.entities.insert(target.id, target);
    for e in a.entities.values_mut() {
        e.fire_on = false;
        e.weapons.truncate(1);
        e.weapons[0].weapon = "cn_yj18".into();
        e.weapons[0].ammo = 4;
        e.weapons[0].mounts = 1;
    }
    for (side, about, point) in [(Side::Blue, 2, 20), (Side::Red, 1, 0)] {
        a.sides[side.index()].contacts.insert(harpoon_core::ContactId(1),serde_json::from_value(json!({"id":1,"about":about,"category":"Ship","estimate":{"x":0,"y":point},"est_course_deg":0,"est_speed_kts":0,"quality":1,"bearing_only":false,"last_update_tick":0,"first_tick":0,"stale":false,"sensors":[],"offset":{"x":0,"y":0}})).unwrap());
    }
    a.log_commands = false;
    let mut b = a.clone();
    let blue = (
        Side::Blue,
        Command::Fire {
            unit: EntityId(1),
            contact: harpoon_core::ContactId(1),
            weapon: "cn_yj18".into(),
            rounds: 2,
        },
    );
    let red = (
        Side::Red,
        Command::Fire {
            unit: EntityId(2),
            contact: harpoon_core::ContactId(1),
            weapon: "cn_yj18".into(),
            rounds: 2,
        },
    );
    sim::tick(&mut a, &[blue.clone(), red.clone()]);
    sim::tick(&mut b, &[red, blue]);
    assert_eq!(serde_json::to_value(a).unwrap(),serde_json::to_value(b).unwrap(),"same planned attacks produce the same projectile identities, rolls and state regardless of cross-side input interleaving");
}

#[test]
fn legacy_observation_default_fields_do_not_rewrite_archive_payloads() {
    let mut w = fixture();
    w.rules = Default::default();
    let raw =
        serde_json::to_value(harpoon_core::observe(&w, harpoon_core::Side::Blue, 0, 0)).unwrap();
    assert!(raw.get("rules_profile").is_none());
    let old: harpoon_core::Observation = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(old).unwrap(), raw);
}

#[test]
fn surfaced_submarine_is_a_legal_surface_missile_target() {
    use harpoon_core::{ContactId, Side};
    let mut w = fixture();
    let shooter = *w.entities.keys().next().unwrap();
    let mut target = w.entity(shooter).unwrap().clone();
    target.id = w.allocate_id();
    target.side = Side::Red;
    target.kind = harpoon_core::types::PlatformKind::Sub;
    target.depth_m = 0.0;
    target.pos = Vec2::new(0.0, 20.0);
    let tid = target.id;
    w.entities.insert(tid, target);
    let s = w.entity_mut(shooter).unwrap();
    s.weapons[0].weapon = "cn_yj18".into();
    s.weapons[0].ammo = 4;
    s.weapons[0].cooldown_s = 0.0;
    w.sides[0].contacts.insert(ContactId(1),serde_json::from_value(json!({"id":1,"about":tid,"category":"Ship","estimate":{"x":0,"y":20},"est_course_deg":0,"est_speed_kts":0,"quality":1,"bearing_only":false,"last_update_tick":0,"first_tick":0,"stale":false,"sensors":[],"offset":{"x":0,"y":0}})).unwrap());
    assert!(
        harpoon_core::weapons::manual_fire(&mut w, Side::Blue, shooter, ContactId(1), "cn_yj18", 1),
        "14.1.2 note: surfaced submarines are surface combatants"
    );
    w.entity_mut(tid).unwrap().depth_m = 30.0;
    w.entity_mut(shooter).unwrap().weapons[0].cooldown_s = 0.0;
    assert!(!harpoon_core::weapons::manual_fire(
        &mut w,
        Side::Blue,
        shooter,
        ContactId(1),
        "cn_yj18",
        1
    ));
}

#[test]
fn unknown_prh_shutdown_modifier_blocks_adjudication_instead_of_inventing_odds() {
    let mut w = fixture();
    let shooter = *w.entities.keys().next().unwrap();
    let mut target = w.entity(shooter).unwrap().clone();
    target.id = w.allocate_id();
    target.side = harpoon_core::Side::Red;
    target.pos = Vec2::new(0.0, 20.0);
    target.weapons.clear();
    target.auto_chaff = false;
    let tid = target.id;
    w.entities.insert(tid, target);
    let def = w.db.weapons["cn_yj91"].clone();
    let missile = harpoon_core::weapons::launch(&mut w, shooter, &def, Some(tid), None).unwrap();
    let hp = w.entity(tid).unwrap().hp;
    w.entity_mut(tid)
        .unwrap()
        .ext
        .insert("emission.radar".into(), 0.0);
    w.entity_mut(missile).unwrap().pos = Vec2::new(0.0, 19.99);
    harpoon_core::weapons::advance_projectiles(&mut w);
    assert_eq!(w.entity(tid).unwrap().hp, hp);
    assert!(
        w.events.iter().any(
            |e| matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule=="adjudication_blocked")
        ),
        "7.4.2 gen3 does not explicitly supply the gen2 shutdown modifier; keep it pending"
    );
    let restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    let pending = &restored.rules.pending_adjudications[&missile];
    assert_eq!(pending.target, tid);
    assert_eq!(
        pending.projectile.projectile.as_ref().unwrap().weapon,
        "cn_yj91"
    );
    let count = w.events.len();
    harpoon_core::weapons::advance_projectiles(&mut w);
    assert_eq!(
        count,
        w.events.len(),
        "pending events cannot roll again each second"
    );
}

#[test]
fn sinking_is_a_saved_countdown_except_submerged_boats_and_explosions() {
    let mut w = fixture();
    let id = *w.entities.keys().next().unwrap();
    w.entity_mut(id).unwrap().hp = 0.0;
    harpoon_core::rules::damage_runtime::step(&mut w);
    assert!(
        w.entity(id).unwrap().alive,
        "14.3 sufficient damage does not make a surface hull disappear immediately"
    );
    let due = w.entity(id).unwrap().ext["damage.sink_at"] as i64;
    assert!(
        (600..=6000).contains(&due) && due % 600 == 0,
        "D10*10 minutes"
    );
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    restored.tick = due - 1;
    harpoon_core::rules::damage_runtime::step(&mut restored);
    assert!(restored.entity(id).unwrap().alive);
    restored.tick = due;
    harpoon_core::rules::damage_runtime::step(&mut restored);
    assert!(!restored.entity(id).unwrap().alive);
    assert_eq!(restored.losses[0], 1);
    for submerged in [false, true] {
        let mut w = fixture();
        let e = w.entity_mut(id).unwrap();
        e.hp = 0.0;
        if submerged {
            e.kind = harpoon_core::types::PlatformKind::Sub;
            e.depth_m = 30.0;
        } else {
            e.ext.insert("damage.instant_sink".into(), 1.0);
        }
        harpoon_core::rules::damage_runtime::step(&mut w);
        assert!(
            !w.entity(id).unwrap().alive,
            "submerged submarines and magazine explosions sink immediately"
        );
    }
}

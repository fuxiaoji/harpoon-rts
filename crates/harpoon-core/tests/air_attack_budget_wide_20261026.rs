//! Shared-budget component boundaries. Claims are trusted provider output;
//! these tests do not claim native observation, defensive fire or physical launch.
use harpoon_core::rules::aircombat2026::budget::{self, AirAttackKind, Claim, Decision};
use harpoon_core::types::PlatformKind;
use harpoon_core::{Database, EntityId, Scenario, World};
use serde_json::json;

fn fixture() -> (World, Vec<EntityId>, EntityId) {
    let mut units = Vec::new();
    for i in 0..14 {
        units.push(
            json!({"side":"Blue","class":"cn_j15","name":format!("A{i:02}"),
            "pos":[0,-20],"course_deg":0,"speed_kts":0,"altitude_m":10000,"fire_on":false}),
        );
    }
    units.push(json!({"side":"Red","class":"cn_001","name":"TARGET","pos":[0,0],"speed_kts":0,"fire_on":false}));
    let scenario: Scenario = serde_json::from_value(
        json!({"id":"shared-budget-component","name":"budget service only",
        "seed":29,"duration_s":1000,"weather":{"visibility_nm":40,"jam":[0,0]},"context":{"rules_profile":"hv2026-rts-v4"},"units":units}),
    )
    .unwrap();
    let w = harpoon_core::load_scenario(
        scenario,
        Database::from_json(include_str!("../../../database/consolidated.json")).unwrap(),
    )
    .unwrap();
    let actors = w
        .entities
        .values()
        .filter(|e| e.name.starts_with('A'))
        .map(|e| e.id)
        .collect();
    let target = w.entities.values().find(|e| e.name == "TARGET").unwrap().id;
    (w, actors, target)
}
fn claim(
    w: &World,
    actor: EntityId,
    target: EntityId,
    kind: AirAttackKind,
    known_kind: PlatformKind,
) -> Claim {
    Claim {
        identity: budget::claim_identity(
            kind,
            actor,
            w.tick,
            &json!({"target":target,"test":"provider output"}),
        ),
        actor,
        target,
        kind,
        known_kind,
    }
}
#[test]
fn budget01_mixed_thirteen_is_atomic_in_both_provider_orders_and_base_is_exempt() {
    for known in [PlatformKind::Ship, PlatformKind::Base] {
        for reverse in [false, true] {
            let (mut w, actors, target) = fixture();
            let mut claims: Vec<_> = actors
                .iter()
                .take(13)
                .enumerate()
                .map(|(i, a)| {
                    claim(
                        &w,
                        *a,
                        target,
                        if i < 6 {
                            AirAttackKind::UnguidedBomb
                        } else {
                            AirAttackKind::Pgm
                        },
                        known,
                    )
                })
                .collect();
            if reverse {
                claims.reverse();
            }
            for c in &claims {
                budget::submit(&mut w, c.clone()).unwrap();
            }
            budget::close_declarations(&mut w);
            for c in &claims {
                assert_eq!(
                    budget::decision(&w, &c.identity),
                    if known == PlatformKind::Ship {
                        Decision::ShipLimit
                    } else {
                        Decision::Reserved
                    }
                );
            }
        }
    }
}
#[test]
fn budget02_same_actor_cross_kind_conflicts_without_a_first_provider_winner() {
    for reverse in [false, true] {
        let (mut w, actors, target) = fixture();
        let mut claims = vec![
            claim(
                &w,
                actors[0],
                target,
                AirAttackKind::UnguidedBomb,
                PlatformKind::Ship,
            ),
            claim(
                &w,
                actors[0],
                target,
                AirAttackKind::Pgm,
                PlatformKind::Ship,
            ),
        ];
        if reverse {
            claims.reverse();
        }
        for c in &claims {
            budget::submit(&mut w, c.clone()).unwrap();
        }
        budget::close_declarations(&mut w);
        for c in &claims {
            assert_eq!(budget::decision(&w, &c.identity), Decision::Conflict);
        }
        assert!(w
            .rules
            .aircombat
            .as_ref()
            .unwrap()
            .surface_last_turn
            .is_empty());
    }
}
#[test]
fn budget03_twelve_tickets_survive_save_are_one_use_and_cancellation_does_not_refund() {
    let (mut w, actors, target) = fixture();
    let claims: Vec<_> = actors
        .iter()
        .take(12)
        .enumerate()
        .map(|(i, a)| {
            claim(
                &w,
                *a,
                target,
                if i < 6 {
                    AirAttackKind::UnguidedBomb
                } else {
                    AirAttackKind::Pgm
                },
                PlatformKind::Ship,
            )
        })
        .collect();
    for c in &claims {
        budget::submit(&mut w, c.clone()).unwrap();
    }
    // The real update owns the declaration barrier and the single normal FCS.
    harpoon_core::weapons::update_weapons(&mut w);
    let epoch = w.rules.aircombat.as_ref().unwrap().surface.scan_epoch;
    for c in &claims {
        budget::offer_ready(&mut w, &c.identity, epoch).unwrap();
    }
    budget::close_ready(&mut w);
    let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    for c in &claims {
        assert_eq!(
            serde_json::to_value(budget::take_ticket(&mut w, &c.identity)).unwrap(),
            serde_json::to_value(budget::take_ticket(&mut saved, &c.identity)).unwrap()
        );
        assert!(budget::take_ticket(&mut w, &c.identity).is_none());
        assert!(budget::take_ticket(&mut saved, &c.identity).is_none());
        budget::finish(&mut w, &c.identity);
        budget::finish(&mut saved, &c.identity);
    }
    assert_eq!(
        serde_json::to_value(&w).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    // Component clock advance only: no native-motion/defense claim is made.
    w.tick += 1;
    for c in &claims {
        budget::cancel_actor(&mut w, c.actor);
    }
    let thirteenth = claim(
        &w,
        actors[12],
        target,
        AirAttackKind::Pgm,
        PlatformKind::Ship,
    );
    budget::submit(&mut w, thirteenth.clone()).unwrap();
    budget::close_declarations(&mut w);
    assert_eq!(
        budget::decision(&w, &thirteenth.identity),
        Decision::ShipLimit
    );
}

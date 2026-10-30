use super::*;
use crate::rules::damage_runtime::{self as damage, ClusterCriticalColumn};
use crate::Rng;

struct Target {
    id: EntityId,
    kind: PlatformKind,
    size: char,
    speed: f64,
    rudder: bool,
}
fn target(w: &World, p: &Prepared, id: EntityId) -> Result<Target, String> {
    let e = w
        .entity(id)
        .filter(|e| e.alive && matches!(e.kind, PlatformKind::Ship | PlatformKind::Base))
        .ok_or("9.6.6 Referee coverage member absent or category changed")?;
    if id == p.target && e.kind != p.known_kind {
        return Err("9.6.6 primary actual category differs from captured own contact".into());
    }
    if e.speed_kts != 0.
        || !e.pos.x.is_finite()
        || !e.pos.y.is_finite()
        || e.pos.range_to(p.point)
            > ac::tables()["surface_attack"]["tolerance_nm"]
                .as_f64()
                .unwrap()
    {
        return Err(
            "9.6.6 actual static adjacency changed; no guessed moving footprint or radius".into(),
        );
    }
    let size = if e.kind == PlatformKind::Base {
        let data = super::super::ground2026::source_target(w, id)?;
        if data.horizontal_armor != 0. || data.vertical_armor != 0. {
            return Err(
                "9.6.6 protected land-target cluster penetration/effect source pending".into(),
            );
        }
        data.size
    } else {
        let data=e.rule_data.as_ref().or_else(||damage::platform(&e.class)).ok_or("9.6.6 full common source hull/armor required, not inferred from a radar or aircraft-operation size")?;
        if !data.deck_cm.is_finite() || !data.belt_cm.is_finite() {
            return Err("9.6.6 source fragment armor unavailable".into());
        }
        data.size
            .chars()
            .next()
            .ok_or("9.6.6 source physical hull size absent")?
    };
    if id != p.target && e.kind == PlatformKind::Ship {
        let turn = w.tick.div_euclid(
            ac::tables()["surface_attack"]["turn_seconds"]
                .as_i64()
                .unwrap(),
        );
        let key = format!("{}:{turn}", id.0);
        if w.rules
            .aircombat
            .as_ref()
            .and_then(|s| s.surface_attacks.get(&key))
            .is_some_and(|actors| actors.len() >= 12 && !actors.contains(&p.actor))
        {
            return Err("9.6.6 secondary-ship collateral versus twelve-aircraft boundary priority source pending; no extra private ship budget".into());
        }
    }
    Ok(Target {
        id,
        kind: e.kind,
        size,
        speed: e.speed_kts,
        rudder: e.ext.get("damage.rudder_jammed").is_some_and(|n| *n > 0.),
    })
}
fn band(weight: f64) -> usize {
    if weight <= tables()["hang_weight_limits_kg"][0].as_f64().unwrap() {
        0
    } else if weight <= tables()["hang_weight_limits_kg"][1].as_f64().unwrap() {
        1
    } else {
        2
    }
}
fn count_expression(size: char, weight: f64) -> Result<&'static str, String> {
    let row = match size {
        'A' | 'B' => "AB",
        'C' | 'D' => "CD",
        'E' | 'F' | 'G' | 'H' => "EH",
        _ => return Err("9.6.6 source critical-count size not printed".into()),
    };
    tables()["critical_counts"][row][band(weight)]
        .as_str()
        .ok_or_else(|| "9.6.6 source critical-count expression missing".into())
}
fn modifier(size: char, weight: f64, large: bool) -> Result<f64, String> {
    let row = match size {
        'A' | 'B' => "AB",
        'C' | 'D' | 'E' => "CE",
        'F' | 'G' => "FG",
        _ => return Err("9.6.6 target size has no printed hit-modifier row".into()),
    };
    tables()["hit_modifier"][row][band(weight)][usize::from(large)]
        .as_f64()
        .ok_or_else(|| "9.6.6 printed cluster modifier absent".into())
}
fn criticals(size: char, weight: f64, large: bool, dice: &[u8]) -> Result<u16, String> {
    let expression = count_expression(size, weight)?;
    let d = u16::from(
        *dice
            .first()
            .ok_or("9.6.6 retained source critical D6 missing")?,
    );
    let raw = if expression == "D6/2+1" {
        if d % 2 != 0 {
            return Err("9.6.6 original D6/2+1 has fractional result; retained source dice, no guessed rounding".into());
        }
        d / 2 + 1
    } else if let Some(add) = expression.strip_prefix("2D6+") {
        d + u16::from(
            *dice
                .get(1)
                .ok_or("9.6.6 second retained source D6 missing")?,
        ) + add
            .parse::<u16>()
            .map_err(|_| "9.6.6 source two-D6 additive expression unavailable")?
    } else if let Some(add) = expression.strip_prefix("D6+") {
        d + add
            .parse::<u16>()
            .map_err(|_| "9.6.6 source D6 additive expression unavailable")?
    } else {
        return Err("9.6.6 unsupported source critical expression; no default zero result".into());
    };
    if large && raw % 2 != 0 {
        return Err("9.6.6 Large pattern half-critical result is fractional; retained original source dice, no rounding/reroll".into());
    }
    Ok(if large { raw / 2 } else { raw })
}
pub(super) fn resolve(
    w: &mut World,
    p: &Prepared,
    no_aa: bool,
    heavy_aa: bool,
) -> Result<(), String> {
    let mut result = json!({"identity":p.identity,"rounds":p.request.rounds,"pattern":p.request.pattern,"coverage":p.request.coverage,"equipment_source":"scenario_laboratory","targets":[],"anchor":p.anchor,"defense":{"no_aa":no_aa,"heavy_aa":heavy_aa},"rts_source":tables()["rts"]["source"]});
    // An actual below-deployment release is a spent dud, before target truth,
    // Ph lookup, area damage or critical dice. Its stock/mass is already gone.
    if p.anchor.altitude_m <= tables()["deployment_dud_at_or_below_m"].as_f64().unwrap() {
        result["resolution"] = json!("automatic_dud");
        w.rules
            .cluster
            .as_mut()
            .unwrap()
            .outcomes
            .insert(p.identity.clone(), result.clone());
        trace(w, p.actor, "9.6.6.source_cluster_attack", result);
        return Ok(());
    }
    let group = w
        .rules
        .cluster
        .as_ref()
        .unwrap()
        .coverages
        .get(&p.request.coverage)
        .cloned()
        .ok_or("9.6.6 Referee static coverage binding unavailable")?;
    let targets: Vec<_> = group
        .members
        .iter()
        .map(|id| target(w, p, *id))
        .collect::<Result<_, _>>()?;
    let row = &tables()["laboratory_weapons"][&p.request.weapon];
    let weight = row["hang_weight_kg"].as_f64().unwrap();
    let large = p.request.pattern == "large";
    let sight: ac::Bombsight =
        serde_json::from_value(tables()["laboratory_fits"][&p.class]["bombsight"].clone())
            .map_err(|_| "9.6.6 explicit experiment sight unavailable")?;
    let mut reports = Vec::new();
    for target in targets {
        let base = ac::unguided_probability(&ac::UnguidedInput {
            target_size: target.size,
            delivery: p.request.delivery,
            altitude_m: p.anchor.altitude_m,
            target_speed_kts: target.speed,
            bombsight: sight,
            heavy_aa,
            no_aa,
            rudder: target.rudder,
            land: target.kind == PlatformKind::Base,
            inertial_lob: false,
        })?;
        let bonus = modifier(target.size, weight, large)?;
        let probability = base + bonus;
        if !probability.is_finite() || !(0. ..=1.).contains(&probability) {
            return Err("9.6.6 cluster-modified final probability exceeds printed supported scope; no unprinted clamp".into());
        }
        let roll_id = format!("{}:target:{}:0", p.identity, target.id.0);
        let saved = w
            .rules
            .cluster
            .as_ref()
            .unwrap()
            .rolls
            .get(&roll_id)
            .cloned();
        let roll = if let Some(saved) = saved {
            saved
        } else {
            let mut rng = w.rng.derive(Rng::hash_tag(
                w.tick,
                p.actor.0,
                &format!("9.6.6:cluster:{}:0", target.id.0),
            ));
            let d100 = 1 + (rng.next_f64() * 100.) as u8;
            let mut dice = Vec::new();
            if target.kind == PlatformKind::Ship && f64::from(d100) <= probability * 100. + 1e-9 {
                dice.push(1 + (rng.next_f64() * 6.) as u8);
                if count_expression(target.size, weight)?.starts_with("2D6+") {
                    dice.push(1 + (rng.next_f64() * 6.) as u8);
                }
            }
            let saved = json!({"d100":d100,"critical_dice":dice,"probability":probability,"weight_kg":weight,"physical_size":target.size,"at":w.tick});
            w.rules
                .cluster
                .as_mut()
                .unwrap()
                .rolls
                .insert(roll_id.clone(), saved.clone());
            saved
        };
        let d100 = roll["d100"]
            .as_u64()
            .ok_or("9.6.6 retained D100 unavailable")? as u8;
        let hit = f64::from(d100) <= probability * 100. + 1e-9;
        let mut count = 0;
        let mut ground_dp = 0.;
        let mut column = None;
        if hit {
            if target.kind == PlatformKind::Ship {
                let dice: Vec<u8> = serde_json::from_value(roll["critical_dice"].clone())
                    .map_err(|_| "9.6.6 retained critical dice unavailable")?;
                count = criticals(target.size, weight, large, &dice)?;
                let selected = match target.size {
                    'A'..='E' => ClusterCriticalColumn::Airburst,
                    'F' | 'G' => ClusterCriticalColumn::SmallCraft,
                    _ => return Err("9.6.6 no printed naval critical column for this size".into()),
                };
                column = Some(if matches!(target.size, 'F' | 'G') {
                    "small_craft"
                } else {
                    "airburst"
                });
                damage::queue_cluster_critical_count(
                    w,
                    p.actor,
                    target.id,
                    &p.request.weapon,
                    count,
                    selected,
                    &roll_id,
                )?;
            } else {
                ground_dp =
                    row["ground_damage_points"].as_f64().unwrap() / if large { 2. } else { 1. };
                let id = w.allocate_id();
                let weapon = w.db.weapons[&p.request.weapon].clone();
                w.rules.aaw.as_mut().unwrap().surface.land_hits.insert(
                    id,
                    super::super::surface2026::LandHit {
                        target: target.id,
                        weapon,
                        shooter_side: p.side,
                        damage: ground_dp,
                    },
                );
            }
        }
        reports.push(json!({"target":target.id,"physical_size":target.size,"base_probability":base,"cluster_modifier":bonus,"probability":probability,"d100":d100,"hit":hit,"critical_dice":roll["critical_dice"],"critical_count":count,"critical_column":column,"direct_damage_points":if target.kind==PlatformKind::Ship{0.}else{ground_dp},"equipment_source":"scenario_laboratory"}));
    }
    result["resolution"] = json!("source_static_author_adjacency");
    result["targets"] = json!(reports);
    result["adjacency_source"] = json!(group.source);
    w.rules
        .cluster
        .as_mut()
        .unwrap()
        .outcomes
        .insert(p.identity.clone(), result.clone());
    trace(w, p.actor, "9.6.6.source_cluster_attack", result);
    Ok(())
}

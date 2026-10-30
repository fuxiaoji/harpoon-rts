use super::*;
use crate::Rng;

pub(super) fn resolve(
    w: &mut World,
    p: &Prepared,
    no_aa: bool,
    heavy_aa: bool,
) -> Result<(), String> {
    let target = w
        .entity(p.target)
        .filter(|t| t.alive && t.kind == p.known_kind)
        .cloned()
        .ok_or("9.6.7 Referee target/category changed after source release")?;
    let tolerance = ac::tables()["surface_attack"]["tolerance_nm"]
        .as_f64()
        .unwrap();
    if !target.pos.x.is_finite()
        || !target.pos.y.is_finite()
        || target.pos.range_to(p.point) > tolerance
        || target.speed_kts != 0.
    {
        return Err("9.6.7 actual moving target/placement differs from captured aim; dynamic rocket intercept source mapping pending".into());
    }
    if target.kind == PlatformKind::Base {
        // PDF107 gives land+2; the explicit PDF108 rocket example applies
        // stationary+2 and Ballistic+1 without that extra land shift. Resolve
        // this source priority before claiming a complete ground consumer.
        return Err("9.6.7 stationary ground attack Land+2 versus PDF108 rocket worked-example modifier priority source pending".into());
    }
    let physical = target
        .rule_data
        .as_ref()
        .or_else(|| super::super::damage_runtime::platform(&target.class))
        .ok_or("9.6.7 Referee source physical size/armor missing")?;
    let size = physical
        .size
        .chars()
        .next()
        .ok_or("9.6.7 source target size missing")?;
    let row = &tables()["laboratory_weapons"][&p.request.weapon];
    let penetration = row["penetration_cm"].as_f64().unwrap();
    if !physical.deck_cm.is_finite() || physical.deck_cm < 0. || physical.deck_cm > penetration {
        return Err("9.6.7 protected-target/nonpenetrating rocket mapping source pending".into());
    }
    let sight: ac::Bombsight =
        serde_json::from_value(tables()["laboratory_fits"][&p.class]["bombsight"].clone())
            .map_err(|_| "9.6.7 explicit laboratory bombsight missing")?;
    let probability = ac::unguided_probability(&ac::UnguidedInput {
        target_size: size,
        delivery: ac::Delivery::Glide,
        altitude_m: p.anchor.altitude_m,
        target_speed_kts: target.speed_kts,
        bombsight: sight,
        heavy_aa,
        no_aa,
        rudder: target
            .ext
            .get("damage.rudder_jammed")
            .is_some_and(|n| *n > 0.),
        land: false,
        inertial_lob: false,
    })?;
    let pairs =
        (p.request.rounds / 2).min(tables()["maximum_pairs_for_ph"].as_u64().unwrap() as u16) as u8;
    let mut rng = w
        .rng
        .derive(Rng::hash_tag(w.tick, p.actor.0, "9.6.7:salvo"));
    let d100 = 1 + (rng.next_f64() * 100.) as u8;
    let any_hit = ac::air_attack_hits(probability, pairs, d100)? > 0;
    let dice = if any_hit {
        Some([
            1 + (rng.next_f64() * 6.) as u8,
            1 + (rng.next_f64() * 6.) as u8,
        ])
    } else {
        None
    };
    let hits = match dice {
        Some(dice) => ac::rocket_hits(p.request.rounds, dice, size)?,
        None => 0,
    };
    let modifier = match size {
        'A' => 2,
        'B' => 1,
        'C' | 'D' => 0,
        'E' => -1,
        'F' => -2,
        'G' => -3,
        'H' => -4,
        _ => return Err("9.6.7 source physical size not printed".into()),
    };
    let dp = row["damage_points"].as_f64().unwrap();
    let weapon = w.db.weapons[&p.request.weapon].clone();
    for n in 0..hits {
        super::super::damage_runtime::queue(
            w,
            p.target,
            &weapon,
            false,
            Some(0.),
            Some(p.side),
            Some(format!("{}:rocket-hit:{n}", p.identity)),
        );
    }
    let result = json!({"identity":p.identity,"actor":p.actor,"target":p.target,"rounds":p.request.rounds,"pairs_for_ph":pairs,"probability":probability,"d100":d100,"dice":dice,"modified_2d6":dice.map(|d|i32::from(d[0])+i32::from(d[1])+modifier),"physical_size":size,"total_hits":hits,"damage_points_per_hit":dp,"penetration_cm":penetration,"defense":{"no_aa":no_aa,"heavy_aa":heavy_aa},"anchor":p.anchor,"aim_source":p.aim_source,"native_measurement":p.native,"equipment_source":"scenario_laboratory","equipment_source_detail":row["source"],"rts_source":tables()["rts"]["source"]});
    w.rules
        .rocket
        .as_mut()
        .unwrap()
        .outcomes
        .insert(p.identity.clone(), result.clone());
    trace(w, p.actor, "9.6.7.source_rocket_attack", result);
    Ok(())
}

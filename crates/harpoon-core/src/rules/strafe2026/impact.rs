use super::*;
use crate::rules::damage_runtime::{self as damage, StrafingCriticalColumn};
use crate::Rng;

fn expression(caliber: f64) -> Result<&'static str, String> {
    let key = if caliber <= 15. {
        "up_to_15mm"
    } else if caliber <= 25. {
        "up_to_25mm"
    } else if caliber >= 26. {
        "26mm_or_more"
    } else {
        return Err("9.6.9 caliber between printed boundaries; no interpolation".into());
    };
    tables()["critical_expressions"][key]
        .as_str()
        .ok_or_else(|| "9.6.9 source caliber critical expression absent".into())
}
fn count(caliber: f64, d6: u8) -> Result<u16, String> {
    if !(1..=6).contains(&d6) {
        return Err("9.6.9 retained source D6 invalid".into());
    }
    if d6 % 2 != 0 {
        return Err("9.6.9 original D6/2 critical count is fractional; original dice retained, no guessed rounding/reroll".into());
    }
    let expr = expression(caliber)?;
    let subtract = if expr == "D6/2" {
        0
    } else {
        expr.strip_prefix("D6/2-")
            .ok_or("9.6.9 unsupported source critical expression")?
            .parse::<i16>()
            .map_err(|_| "9.6.9 source critical subtraction unavailable")?
    };
    let resolved = i16::from(d6 / 2) - subtract;
    if resolved < 0 {
        return Err(
            "9.6.9 source negative critical result handling pending, no invented clamp".into(),
        );
    }
    Ok(resolved as u16)
}
pub(super) fn resolve(
    w: &mut World,
    p: &Prepared,
    gun: &WeaponDef,
    no_aa: bool,
    heavy_aa: bool,
) -> Result<Value, String> {
    // The source pass is already committed; private truth never authorizes it.
    let target = w
        .entity(p.target)
        .filter(|t| t.alive && t.kind == p.known_kind)
        .cloned()
        .ok_or("9.6.9 actual target/category differs from own captured target")?;
    if !target.pos.x.is_finite()
        || !target.pos.y.is_finite()
        || target.speed_kts != 0.
        || target.pos.range_to(p.point)
            > ac::tables()["surface_attack"]["tolerance_nm"]
                .as_f64()
                .unwrap()
    {
        return Err(
            "9.6.9 moving target/overflight placement beyond captured static source scope".into(),
        );
    }
    let (size, column, column_name) = if target.kind == PlatformKind::Base {
        let data = super::super::ground2026::source_target(w, target.id)?;
        if data.horizontal_armor != 0. || data.vertical_armor != 0. {
            return Err("9.6.9 real H3 penetration missing for protected ground location; zero-armor scope only".into());
        }
        (data.size, StrafingCriticalColumn::Ground, "ground_function")
    } else if target.kind == PlatformKind::Ship {
        let data = target
            .rule_data
            .as_ref()
            .or_else(|| damage::platform(&target.class))
            .ok_or("9.6.9 source naval size/deck/belt/critical role required")?;
        if !data.deck_cm.is_finite()
            || !data.belt_cm.is_finite()
            || data.deck_cm != 0.
            || data.belt_cm != 0.
            || data
                .critical_armor
                .values()
                .any(|armor| !armor.is_finite() || *armor != 0.)
        {
            return Err("9.6.9 real H3 penetration missing for positive/unknown deck/belt/CHP armor; no implicit penetration value".into());
        }
        let size = data
            .size
            .chars()
            .next()
            .ok_or("9.6.9 physical naval size absent")?;
        let column = match size {
            'A'..='E' => StrafingCriticalColumn::Airburst,
            'F' | 'G' => StrafingCriticalColumn::SmallCraft,
            _ => return Err("9.6.9 naval H/unknown critical column has no printed binding".into()),
        };
        (
            size,
            column,
            if matches!(size, 'F' | 'G') {
                "small_craft"
            } else {
                "airburst"
            },
        )
    } else {
        return Err("9.6.9 actual target is not naval/ground".into());
    };
    let sight: ac::Bombsight =
        serde_json::from_value(tables()["aircraft"][&p.class]["bombsight"].clone())
            .map_err(|_| "9.6.9 source fitted bombsight missing")?;
    let probability = ac::unguided_probability(&ac::UnguidedInput {
        target_size: size,
        delivery: Delivery::Strafe,
        altitude_m: p.anchor.altitude_m,
        target_speed_kts: target.speed_kts,
        bombsight: sight,
        heavy_aa,
        no_aa,
        rudder: target
            .ext
            .get("damage.rudder_jammed")
            .is_some_and(|n| *n > 0.),
        land: target.kind == PlatformKind::Base,
        inertial_lob: false,
    })?;
    let roll=w.rules.strafe.as_ref().unwrap().rolls.get(&p.identity).cloned().unwrap_or_else(||{
        let mut rng=w.rng.derive(Rng::hash_tag(w.tick,p.actor.0,"9.6.9:strafe"));let d100=1+(rng.next_f64()*100.)as u8;
        let d6=if f64::from(d100)<=probability*100.+1e-9{Some(1+(rng.next_f64()*6.)as u8)}else{None};
        json!({"d100":d100,"critical_d6":d6,"fired_at":w.tick,"probability":probability,"caliber_mm":gun.rule_data.as_ref().unwrap().diameter_mm})
    });
    w.rules
        .strafe
        .as_mut()
        .unwrap()
        .rolls
        .insert(p.identity.clone(), roll.clone());
    let d100 = roll["d100"]
        .as_u64()
        .ok_or("9.6.9 retained source D100 unavailable")? as u8;
    let hit = ac::air_attack_hits(probability, 1, d100)? > 0;
    let critical_count = if hit {
        count(
            gun.rule_data.as_ref().unwrap().diameter_mm,
            roll["critical_d6"]
                .as_u64()
                .ok_or("9.6.9 retained critical D6 unavailable")? as u8,
        )?
    } else {
        0
    };
    if critical_count > 0 {
        damage::queue_strafing_critical_count(
            w,
            p.actor,
            p.target,
            gun,
            critical_count,
            column,
            &p.identity,
        )?;
    }
    Ok(
        json!({"identity":p.identity,"actor":p.actor,"target":p.target,"gun":p.gun,"gun_rating":p.gun_rating,"caliber_mm":gun.rule_data.as_ref().unwrap().diameter_mm,"source":gun.rule_data.as_ref().unwrap().source,"salvo":1,"probability":probability,"d100":d100,"hit":hit,"critical_d6":roll["critical_d6"],"critical_count":critical_count,"critical_column":column_name,"direct_damage_points":0.0,"ammunition_model":tables()["ammunition_model"],"physical_round_count":Value::Null,"defense":{"no_aa":no_aa,"heavy_aa":heavy_aa},"anchor":p.anchor,"aim_source":p.aim_source,"native_measurement":p.native,"rts_source":tables()["rts"]["source"]}),
    )
}

//! Harpoon V2026 source surface weapon procedures; explicit v4 only.
pub mod antiship;
pub mod gunnery;
use crate::{db::WeaponDef, EntityId, EventKind, Visibility, World};
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::OnceLock};
pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/surface-2026.json")).unwrap()
    })
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct State {
    pub land_outcomes: BTreeMap<EntityId, LandOutcome>,
    pub land_hits: BTreeMap<EntityId, LandHit>,
    pub antiship: antiship::State,
    #[serde(default,skip_serializing_if="gunnery::State::is_empty")]
    pub gunnery: gunnery::State,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LandOutcome {
    pub tick: i64,
    pub probability: f64,
    pub d100: u8,
    pub hit: bool,
    pub armor_cm: Option<f64>,
    pub penetrated: Option<bool>,
    pub damage: Option<f64>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LandHit {
    pub target: EntityId,
    pub weapon: WeaponDef,
    pub shooter_side: crate::Side,
    pub damage: f64,
}
/// Consume §11.2 at the actual terminal point. A known miss is final; a hit
/// with unresolved damage retains its die and the complete flight object.
pub fn land_terminal(
    w: &mut World,
    projectile: EntityId,
    def: &WeaponDef,
    data: &crate::world::ProjectileData,
) -> Result<(), String> {
    let target = data.target.ok_or("11.2 fixed land target missing")?;
    let target_data = super::ground2026::source_target(w, target)?;
    let spec = super::ground2026::source_missile(w, &def.id)?;
    if spec.nuclear {
        return Err("11.2 nuclear hit is automatic but nuclear damage requires its separate source executor".into());
    }
    let prior = w
        .rules
        .aaw
        .as_ref()
        .and_then(|s| s.surface.land_outcomes.get(&projectile))
        .cloned();
    let mut outcome = if let Some(o) = prior {
        o
    } else {
        let probability = super::ground2026::terminal_probability(w, &def.id, target)?;
        let mut r = w
            .rng
            .derive(crate::Rng::hash_tag(0, projectile.0, "11.2.land_terminal"));
        let d100 = 1 + (r.next_f64() * 100.) as u8;
        LandOutcome {
            tick: w.tick,
            probability,
            d100,
            hit: d100 as f64 <= probability * 100.,
            armor_cm: None,
            penetrated: None,
            damage: None,
        }
    };
    w.rules
        .aaw
        .as_mut()
        .ok_or("11 source runtime missing")?
        .surface
        .land_outcomes
        .insert(projectile, outcome.clone());
    super::aaw2026::trace(
        w,
        data.launched_by,
        "11.2.source_terminal",
        json!({"projectile":projectile,"target":target,"outcome":outcome}),
    );
    if !outcome.hit {
        return Ok(());
    }
    let weapon = def
        .rule_data
        .as_ref()
        .or_else(|| super::damage_runtime::weapon(&def.id))
        .filter(|s| !s.source.trim().is_empty())
        .ok_or("11.1 source weapon DP/penetration/profile missing")?;
    if !weapon.airburst.is_empty() || weapon.shaped_charge {
        return Err(
            "11.1 land airburst/shaped-charge damage requires its source-specific executor".into(),
        );
    }
    let base = weapon
        .ship_dp
        .filter(|d| d.is_finite() && *d >= 0.)
        .ok_or("11.1 source conventional HE damage missing")?;
    let profile = if !weapon.terminal_profile.is_empty() {
        &weapon.terminal_profile
    } else {
        &weapon.profile
    };
    let armor = match profile.as_str() {
        "skim" => target_data.vertical_armor,
        "level" | "dive" | "high" => target_data.horizontal_armor,
        _ => {
            return Err(
                "11.1/14.1.6 source impact trajectory not bound to vertical/horizontal armor"
                    .into(),
            )
        }
    };
    let penetrated = armor == 0.
        || weapon
            .penetration_cm
            .filter(|p| p.is_finite())
            .ok_or("11.1 source penetration missing")?
            > armor;
    let damage = if penetrated { base } else { base / 2. };
    outcome.armor_cm = Some(armor);
    outcome.penetrated = Some(penetrated);
    outcome.damage = Some(damage);
    w.rules
        .aaw
        .as_mut()
        .unwrap()
        .surface
        .land_outcomes
        .insert(projectile, outcome.clone());
    // §11.1.2 only defines normal ratio criticals for structures. Do not
    // silently map the §14.1.5 ship airframe/fire effects onto a surviving
    // land target. Direct destruction makes that additional question moot.
    let destroyed = w
        .entity(target)
        .is_some_and(|e| target_data.damage_points.is_some() && damage >= e.hp);
    if !weapon.extra.is_empty() && !destroyed {
        return Err("11.1.2/14.1.5 missile additional-critical scope for surviving land target unresolved; terminal die retained".into());
    }
    if !penetrated && !destroyed {
        return Err("11.1.2 nonpenetrating hit requires scenario critical protection mapping; terminal die retained".into());
    }
    w.rules.aaw.as_mut().unwrap().surface.land_hits.insert(
        projectile,
        LandHit {
            target,
            weapon: def.clone(),
            shooter_side: data.launched_by_side,
            damage,
        },
    );
    Ok(())
}
/// Resolve source land DP together at the end of projectile movement, so
/// the normal damage-ratio die sees this tick's complete damage total.
pub fn flush_land_hits(w: &mut World) {
    let hits = w
        .rules
        .aaw
        .as_mut()
        .map(|s| std::mem::take(&mut s.surface.land_hits))
        .unwrap_or_default();
    let mut groups: BTreeMap<EntityId, Vec<(EntityId, LandHit)>> = BTreeMap::new();
    for (id, hit) in hits {
        groups.entry(hit.target).or_default().push((id, hit));
    }
    for (target, hits) in groups {
        let Some(victim) = w.entity(target).cloned() else {
            continue;
        };
        let total: f64 = hits.iter().map(|(_, h)| h.damage).sum();
        let identity = hits
            .iter()
            .map(|(id, _)| id.0.to_string())
            .collect::<Vec<_>>()
            .join(":");
        let criticals = super::damage_runtime::land_ratio_criticals(
            w,
            target,
            total,
            &format!("11.batch:{identity}"),
        );
        for (i, (id, hit)) in hits.iter().enumerate() {
            let result = super::ground2026::apply_damage(
                w,
                target,
                hit.damage,
                if i == 0 { criticals } else { 0 },
                &format!("11.projectile:{}", id.0),
            );
            if let Err(reason) = result {
                super::aaw2026::blocked(w, target, "11.damage_pending", &reason);
                continue;
            }
            let hp = w.entity(target).map(|e| e.hp).unwrap_or(0.);
            w.emit(
                EventKind::Impact {
                    side: victim.side,
                    unit: target,
                    unit_name: victim.name.clone(),
                    weapon: hit.weapon.name.clone(),
                    damage: hit.damage,
                    hp_left: hp,
                },
                Visibility::Side(victim.side),
            );
            crate::damage::bda_to_shooter(
                w,
                target,
                &hit.weapon,
                hit.damage,
                hp,
                Some(hit.shooter_side),
            );
        }
    }
}
pub fn defer_land(w: &mut World, projectile: EntityId, target: EntityId, reason: &str) {
    if let Some(p) = w.entity(projectile).cloned() {
        let evidence = w.events.iter().filter(|e| matches!(&e.kind,EventKind::RulesEvent{rule,..} if rule == "11.2.source_terminal")).cloned().collect();
        w.rules.pending_adjudications.entry(projectile).or_insert(
            super::damage_runtime::PendingAdjudication {
                tick: w.tick,
                rule: format!("11.source_land: {reason}"),
                target,
                projectile: p,
                evidence,
            },
        );
        super::aaw2026::blocked(w, projectile, "11.terminal_pending", reason);
    }
}

//! §9.6.6 resolved cluster critical counts join the existing common phase.
//! The ordnance consumer has already rolled its printed damage expression.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterCriticalColumn {
    Airburst,
    SmallCraft,
    Ground,
}
pub type StrafingCriticalColumn = ClusterCriticalColumn;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CountedCriticalOrigin {
    #[default]
    ClusterBomb,
    AircraftGun,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClusterCriticalCount {
    pub target: EntityId,
    pub shooter: Side,
    pub weapon: String,
    pub count: u16,
    pub column: ClusterCriticalColumn,
    #[serde(default)]
    pub origin: CountedCriticalOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gun_caliber_mm: Option<f64>,
}
pub(super) fn get<'a>(w: &'a World, h: &Hit) -> Option<&'a ClusterCriticalCount> {
    w.rules
        .cluster_critical_counts
        .get(&format!("{}:{}", h.target.0, h.identity))
}
pub(crate) fn queue_cluster_critical_count(
    w: &mut World,
    shooter: EntityId,
    target: EntityId,
    weapon: &str,
    count: u16,
    column: ClusterCriticalColumn,
    attack_id: &str,
) -> Result<(), String> {
    let def =
        w.db.weapons
            .get(weapon)
            .filter(|d| d.kind == WeaponKind::Bomb)
            .cloned()
            .ok_or("9.6.6 actual cluster bomb definition absent")?;
    if column == ClusterCriticalColumn::Ground {
        return Err("9.6.6 land cluster DP uses the land-hit batch".into());
    }
    queue_count(
        w,
        shooter,
        target,
        def,
        count,
        column,
        CountedCriticalOrigin::ClusterBomb,
        attack_id,
    )
}
pub(crate) fn queue_strafing_critical_count(
    w: &mut World,
    shooter: EntityId,
    target: EntityId,
    gun: &WeaponDef,
    count: u16,
    column: StrafingCriticalColumn,
    attack_id: &str,
) -> Result<(), String> {
    let actor = w
        .entity(shooter)
        .filter(|e| e.alive && e.kind == PlatformKind::Aircraft)
        .ok_or("9.6.9 actual live aircraft gun-pass body required")?;
    if gun.kind != WeaponKind::Gun
        || !super::super::aircombat2026::tables()["profiles"][&actor.class]["gun"]
            .as_f64()
            .is_some_and(|v| v > 0.)
    {
        return Err(
            "9.6.9 source fitted aircraft gun rating and real Gun-kind abstraction required".into(),
        );
    }
    let spec = weapon_spec(gun);
    if spec.source.trim().is_empty() || !spec.diameter_mm.is_finite() || spec.diameter_mm <= 0. {
        return Err(
            "9.6.9 cited actual gun caliber required; no fabricated ammunition or penetration"
                .into(),
        );
    }
    queue_count(
        w,
        shooter,
        target,
        gun.clone(),
        count,
        column,
        CountedCriticalOrigin::AircraftGun,
        attack_id,
    )
}
fn queue_count(
    w: &mut World,
    shooter: EntityId,
    target: EntityId,
    mut def: WeaponDef,
    count: u16,
    column: ClusterCriticalColumn,
    origin: CountedCriticalOrigin,
    attack_id: &str,
) -> Result<(), String> {
    if w.rules.profile != RuleProfile::Rts2026Full || attack_id.trim().is_empty() || count == 0 {
        return Err(
            "9.6.6 actual resolved positive critical count and stable physical identity required"
                .into(),
        );
    }
    let side = w
        .entity(shooter)
        .ok_or("9.6.6 actual release platform absent")?
        .side;
    let identity = short_tag(attack_id);
    let key = format!("{}:{identity}", target.0);
    let metadata = ClusterCriticalCount {
        target,
        shooter: side,
        weapon: def.id.clone(),
        count,
        column,
        origin,
        gun_caliber_mm: (origin == CountedCriticalOrigin::AircraftGun)
            .then(|| weapon_spec(&def).diameter_mm),
    };
    if let Some(old) = w.rules.cluster_critical_counts.get(&key) {
        return if old == &metadata {
            Ok(())
        } else {
            Err("9.6.6 cluster hit identity reused with different physical inputs".into())
        };
    }
    let t = w
        .entity(target)
        .filter(|t| t.alive)
        .ok_or("9.6.6/9.6.9 actual living target required for critical-count consumer")?;
    if column == ClusterCriticalColumn::Ground {
        if origin != CountedCriticalOrigin::AircraftGun
            || t.kind != PlatformKind::Base
            || count > u16::from(u8::MAX)
        {
            return Err("9.6.9 actual ground point target and exact count within source receiver range required".into());
        }
        let f = super::super::ground2026::source_target(w, target)?;
        if !f.horizontal_armor.is_finite() || f.horizontal_armor != 0. {
            return Err("9.6.9 gun penetration is not bound for a protected ground target".into());
        }
    } else {
        if t.kind != PlatformKind::Ship {
            return Err("naval counted criticals require an actual ship".into());
        }
        let physical =
            platform_of(t).ok_or("9.6.6/9.6.9 source target size/armor/critical role absent")?;
        if origin == CountedCriticalOrigin::AircraftGun
            && (physical.deck_cm != 0. || physical.belt_cm != 0.)
        {
            return Err("9.6.9 no invented gun penetration for a protected naval target".into());
        }
        match column {
        ClusterCriticalColumn::Airburst if matches!(physical.size.as_str(),"A"|"B"|"C"|"D"|"E")=>{},
        ClusterCriticalColumn::SmallCraft if matches!(physical.size.as_str(),"F"|"G")
            &&matches!(role(t),"small_combatant"|"small_cargo")=>{},
        _=>return Err("9.6.6 source size and Airburst/SmallCraft sub-column do not match; H or unknown role remains pending".into()),
    }
    }
    if w.rules.resolved_hits.contains(&key) {
        return Err("9.6.6 cluster identity collides with another committed hit".into());
    }
    def.damage = 0.;
    let mut spec = weapon_spec(&def);
    spec.ship_dp = Some(0.);
    spec.sub_dp = Some(0.);
    spec.extra.clear();
    spec.airburst.clear();
    spec.fuze = if origin == CountedCriticalOrigin::ClusterBomb {
        "cluster_fragmentation"
    } else {
        "aircraft_gun_pass"
    }
    .into();
    spec.source=if origin==CountedCriticalOrigin::ClusterBomb {"English2026 PDF106/9-5 and108/9-7 section9.6.6: already resolved count, no new damage dice or ordinary DP"}else{"English2026 PDF109/9-8 section9.6.9: actual calibrated abstract gun pass; resolved critical count, no ammunition/penetration or direct DP invented"}.into();
    def.rule_data = Some(spec);
    w.rules
        .cluster_critical_counts
        .insert(key.clone(), metadata);
    w.rules.resolved_hits.insert(key);
    register_source_cause(w, target, &identity);
    register_private_hit(w, target, &identity);
    w.rules.pending.push(Hit {
        target,
        weapon: def,
        underwater: false,
        bearing: None,
        shooter: Some(side),
        identity,
        tick: w.tick,
        secondary: false,
        torpedo_deep: false,
        impact_area: None,
        targeted_sensor: None,
        mine: None,
        projector: None,
        navigation: None,
        asw: None,
    });
    if !w.rules.in_batch {
        finish(w);
    }
    Ok(())
}

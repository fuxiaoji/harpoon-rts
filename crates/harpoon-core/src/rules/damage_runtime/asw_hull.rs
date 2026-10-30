//! Source §8.5 real pressure-hull probability and contact submunition provenance.
use super::*;
use serde_json::{json, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AswSubSize {
    #[serde(rename = "vsmall")]
    VSmall,
    Small,
    Medium,
    Large,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AswHullDesign {
    Single,
    Double,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AswHullFit {
    pub source: String,
    #[serde(default)]
    pub size: Option<AswSubSize>,
    #[serde(default)]
    pub design: Option<AswHullDesign>,
    #[serde(default)]
    pub in_service_year: Option<u16>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundAswHull {
    pub class: String,
    pub fit: AswHullFit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AswContactScale {
    Full,
    Half,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AswHydrostaticKind {
    Projector,
    DepthCharge,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AswImpact {
    /// Contact DP is always inflicted; the independent source pressure trial
    /// may succeed, fail, or remain pending for missing hull characteristics.
    ProjectorContact { penetrated: Option<bool> },
    DepthChargeSelfBlast {
        reason: String,
    },
    Hydrostatic {
        kind: AswHydrostaticKind,
    },
    ClusterContact {
        penetrated: bool,
        scale: AswContactScale,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AswContactOutcome {
    pub identity: String,
    pub pressure_roll: Option<u8>,
    pub chance_percent: Option<f64>,
    pub penetrated: Option<bool>,
    pub damage_queued: bool,
    pub minor_damage_queued: bool,
    pub hull_effect_queued: bool,
    pub pending_reason: Option<String>,
}
pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../../data/rules/asw-hull-2026.json"))
            .expect("source ASW hull table")
    })
}
pub fn initialize_asw_hulls(w: &mut World) -> Result<(), String> {
    if w.rules.profile != RuleProfile::Rts2026Full {
        return Ok(());
    }
    let fits = w
        .context
        .asw_weapons_2026
        .as_ref()
        .map(|c| c.submarine_hulls.clone())
        .unwrap_or_default();
    let mut bound = BTreeMap::new();
    for (name, fit) in fits {
        let found: Vec<_> = w.entities.values().filter(|e| e.name == name).collect();
        if found.len() != 1
            || found[0].kind != PlatformKind::Sub
            || fit.source.trim().is_empty()
            || fit.in_service_year == Some(0)
        {
            return Err("8.5 exact unique actual submarine and cited hull fit/year required; unknown year is None, not zero".into());
        }
        let e = found[0];
        bound.insert(
            e.id,
            BoundAswHull {
                class: e.class.clone(),
                fit,
            },
        );
    }
    w.rules.asw_hulls = bound;
    Ok(())
}
fn chance(w: &World, target: EntityId, scale: AswContactScale) -> Result<f64, String> {
    let e = w
        .entity(target)
        .filter(|e| e.kind == PlatformKind::Sub)
        .ok_or("8.5 pressure target is not an actual submarine")?;
    let b=w.rules.asw_hulls.get(&target).filter(|b|b.class==e.class).ok_or("8.5 source four-band submarine hull size/design/year or original body binding absent; no A-H inference")?;
    let f = &b.fit;
    if f.source.trim().is_empty() {
        return Err("8.5 hull fit source missing".into());
    }
    let size = f
        .size
        .ok_or("8.5 source four-band submarine size unknown")?;
    let design = f
        .design
        .ok_or("8.5 source single/double hull design unknown")?;
    let year = f
        .in_service_year
        .filter(|v| *v > 0)
        .ok_or("8.5 source class in-service year unknown")?;
    let column = match size {
        AswSubSize::VSmall => 0,
        AswSubSize::Small => 1,
        AswSubSize::Medium => 2,
        AswSubSize::Large => 3,
    };
    let row = match design {
        AswHullDesign::Single => "single",
        AswHullDesign::Double => "double",
    };
    let early = year < u16::try_from(tables()["pre1955_before_year"].as_u64().unwrap()).unwrap();
    let mut c = tables()[row][column].as_f64().unwrap();
    if early {
        c += tables()["pre1955_add_points"].as_f64().unwrap();
    }
    if scale == AswContactScale::Half {
        if early {
            return Err("8.5 half-pressure chance combined with pre1955 bonus order lacks verified priority".into());
        }
        c *= 0.5;
        if c.fract() != 0. {
            return Err(
                "8.5 fractional half-pressure D100 percent rounding not source verified".into(),
            );
        }
    }
    Ok(c)
}
fn trial(
    w: &mut World,
    t: &Entity,
    r: &mut Rng,
    identity: &str,
    scale: AswContactScale,
) -> Result<(f64, u8, bool), String> {
    let chance = chance(w, t.id, scale)?;
    let roll = die(w, t.id, r, 100, identity, "8.5.contact_pressure_hull");
    let penetrated = f64::from(roll) <= chance;
    trace(
        w,
        t.id,
        "8.5.contact_pressure_hull",
        json!({"hit":identity,"chance_percent":chance,"effective_percent":chance.min(100.),"roll":roll,"penetrated":penetrated,"scale":scale,"source":"English2026 PDF100/8-20; explicit bound four-band hull data"}),
    );
    Ok((chance, roll, penetrated))
}
pub(super) fn hydrostatic(h: &Hit) -> bool {
    matches!(h.asw, Some(AswImpact::Hydrostatic { .. })) || h.projector.is_some()
}
pub(super) fn allow_critical(
    w: &mut World,
    t: &Entity,
    h: &Resolved,
    r: &mut Rng,
    identity: &str,
) -> bool {
    if matches!(h.hit.asw,Some(AswImpact::ProjectorContact{..})) {
        // One independent pressure trial per actual charge, already performed
        // by the contact producer. A random HullPen slot cannot bypass it.
        return false;
    }
    if w.rules.profile != RuleProfile::Rts2026Full || !hydrostatic(&h.hit) {
        return true;
    }
    match trial(w, t, r, identity, AswContactScale::Full) {
        Ok((_, _, passed)) => passed,
        Err(reason) => {
            super::super::expanded_damage2026::source_pending(
                w,
                t.id,
                identity,
                "asw_pressure_hull",
                &reason,
            );
            false
        }
    }
}
/// §8.5.3 contact charges inflict their listed DP regardless of the separate
/// pressure-hull trial. This is deliberately separate from RBK submunitions.
pub(crate) fn queue_asw_projector_contact(
    w: &mut World, shooter: EntityId, target: EntityId, weapon: &str,
    contact_dp: f64, attack_id: &str,
) -> Result<AswContactOutcome,String> {
    if w.rules.profile!=RuleProfile::Rts2026Full || attack_id.trim().is_empty()
        || !contact_dp.is_finite() || contact_dp<=0. {
        return Err("8.5.3 actual contact identity and positive finite Contact DP required".into());
    }
    let identity=short_tag(attack_id);
    let key=format!("{}:{identity}",target.0);
    let inputs=json!({"kind":"projector_contact","shooter":shooter,"target":target,"weapon":weapon,"contact_dp":contact_dp});
    if let Some(old)=w.rules.asw_contact_inputs.get(&key) {
        if old!=&inputs {return Err("8.5 contact identity reused with different physical inputs".into());}
        return w.rules.asw_contact_results.get(&key).cloned().ok_or_else(||"8.5 contact outcome cache incomplete".into());
    }
    let side=w.entity(shooter).ok_or("8.5.3 actual release platform absent")?.side;
    let mut def=w.db.weapons.get(weapon).filter(|d|d.kind==WeaponKind::AswRocket
        &&d.guidance==crate::db::Guidance::AswBallistic).cloned()
        .ok_or("8.5.3 actual source projector charge definition absent")?;
    let mut outcome=AswContactOutcome {identity:identity.clone(),pressure_roll:None,chance_percent:None,
        penetrated:None,damage_queued:false,minor_damage_queued:false,hull_effect_queued:false,pending_reason:None};
    if let Some(t)=w.entity(target).filter(|t|t.alive&&t.kind==PlatformKind::Sub&&t.depth_m>0.).cloned() {
        if w.rules.resolved_hits.contains(&key) {return Err("8.5 contact collides with a committed hit identity".into());}
        let mut r=w.rng.derive(Rng::hash_tag(w.tick,target.0,&format!("8.5.projector_contact:{identity}")));
        match trial(w,&t,&mut r,&identity,AswContactScale::Full) {
            Ok((chance,roll,penetrated))=>{
                outcome.chance_percent=Some(chance);outcome.pressure_roll=Some(roll);
                outcome.penetrated=Some(penetrated);outcome.hull_effect_queued=penetrated;
            },
            Err(reason)=>{
                super::super::expanded_damage2026::source_pending(w,target,&identity,"asw_pressure_hull",&reason);
                outcome.pending_reason=Some(reason);
            }
        }
        def.damage=contact_dp;
        let mut spec=weapon_spec(&def);
        spec.ship_dp=Some(contact_dp);spec.sub_dp=Some(contact_dp);
        spec.extra.clear();spec.airburst.clear();spec.fuze="asw_projector_contact".into();
        spec.source="English2026 PDF99–100/8.5.3: each contact inflicts listed DP plus one independent pressure trial".into();
        def.rule_data=Some(spec);
        w.rules.resolved_hits.insert(key.clone());
        register_source_cause(w,target,&identity);register_private_hit(w,target,&identity);
        w.rules.pending.push(Hit{target,weapon:def,underwater:true,bearing:None,shooter:Some(side),
            identity:identity.clone(),tick:w.tick,secondary:false,torpedo_deep:false,impact_area:None,
            targeted_sensor:None,mine:None,navigation:None,projector:None,
            asw:Some(AswImpact::ProjectorContact{penetrated:outcome.penetrated})});
        outcome.damage_queued=true;
    } else {
        outcome.pending_reason=Some("8.5.3 actual submerged target unavailable after physical release".into());
    }
    w.rules.asw_contact_inputs.insert(key.clone(),inputs);
    w.rules.asw_contact_results.insert(key,outcome.clone());
    if outcome.damage_queued&&!w.rules.in_batch {finish(w);}
    Ok(outcome)
}
/// Real direct contact queues either pressure-hull effect or ordinary MinorDP.
/// No fake torpedo/mine/hydrostatic provenance and no extra Major blast radius.
pub(crate) fn queue_asw_cluster_contact(
    w: &mut World,
    shooter: EntityId,
    target: EntityId,
    weapon: &str,
    minor_dp: f64,
    scale: AswContactScale,
    attack_id: &str,
) -> Result<AswContactOutcome, String> {
    if w.rules.profile != RuleProfile::Rts2026Full
        || attack_id.trim().is_empty()
        || !minor_dp.is_finite()
        || minor_dp <= 0.
    {
        return Err(
            "8.5 actual source cluster contact and positive finite Minor DP required".into(),
        );
    }
    let identity = short_tag(attack_id);
    let key = format!("{}:{identity}", target.0);
    let inputs = json!({"shooter":shooter,"target":target,"weapon":weapon,"minor_dp":minor_dp,"scale":scale});
    if let Some(old) = w.rules.asw_contact_inputs.get(&key) {
        if old != &inputs {
            return Err("8.5 actual contact identity reused with different physical inputs".into());
        }
        return w
            .rules
            .asw_contact_results
            .get(&key)
            .cloned()
            .ok_or_else(|| "8.5 contact outcome cache incomplete".into());
    }
    let side = w
        .entity(shooter)
        .ok_or("8.5 actual cluster release platform absent")?
        .side;
    let mut def =
        w.db.weapons
            .get(weapon)
            .filter(|d| d.kind == WeaponKind::Bomb)
            .cloned()
            .ok_or("8.5 actual source cluster bomb definition absent")?;
    let mut outcome = AswContactOutcome {
        identity: identity.clone(),
        pressure_roll: None,
        chance_percent: None,
        penetrated: None,
        damage_queued: false,
        minor_damage_queued: false,
        hull_effect_queued: false,
        pending_reason: None,
    };
    let t = w
        .entity(target)
        .filter(|e| e.alive && e.kind == PlatformKind::Sub && e.depth_m > 0.)
        .cloned();
    if let Some(t) = t {
        let mut r = w.rng.derive(Rng::hash_tag(
            w.tick,
            target.0,
            &format!("8.5.cluster_contact:{identity}"),
        ));
        match trial(w, &t, &mut r, &identity, scale) {
            Err(reason) => {
                super::super::expanded_damage2026::source_pending(
                    w,
                    target,
                    &identity,
                    "asw_pressure_hull",
                    &reason,
                );
                outcome.pending_reason = Some(reason);
            }
            Ok((chance, roll, penetrated)) => {
                outcome.chance_percent = Some(chance);
                outcome.pressure_roll = Some(roll);
                outcome.penetrated = Some(penetrated);
                if !w.rules.resolved_hits.insert(key.clone()) {
                    return Err(
                        "8.5 physical contact identity collides with an already committed hit"
                            .into(),
                    );
                }
                let dp = if penetrated { 0. } else { minor_dp };
                def.damage = dp;
                let mut spec = weapon_spec(&def);
                spec.sub_dp = Some(dp);
                spec.ship_dp = Some(dp);
                spec.extra.clear();
                spec.airburst.clear();
                spec.fuze = "contact".into();
                spec.source="English2026 PDF101/8-21; real ASW cluster submunition contact, pressure success or Minor-only failure".into();
                def.rule_data = Some(spec);
                register_source_cause(w, target, &identity);
                register_private_hit(w, target, &identity);
                w.rules.pending.push(Hit {
                    target,
                    weapon: def,
                    underwater: true,
                    bearing: None,
                    shooter: Some(side),
                    identity: identity.clone(),
                    tick: w.tick,
                    secondary: false,
                    torpedo_deep: false,
                    impact_area: None,
                    targeted_sensor: None,
                    mine: None,
                    navigation: None,
                    projector: None,
                    asw: Some(AswImpact::ClusterContact { penetrated, scale }),
                });
                outcome.damage_queued = true;
                outcome.minor_damage_queued = !penetrated;
                outcome.hull_effect_queued = penetrated;
            }
        }
    } else {
        outcome.pending_reason=Some("8.5 referee actual target unavailable/submerged physical placement unresolved after release".into());
    }
    w.rules.asw_contact_inputs.insert(key.clone(), inputs);
    w.rules.asw_contact_results.insert(key, outcome.clone());
    if outcome.damage_queued && !w.rules.in_batch {
        finish(w)
    }
    Ok(outcome)
}

#[cfg(test)]
#[path = "../damage_runtime_asw_hull_following_20261026.rs"]
mod tests;

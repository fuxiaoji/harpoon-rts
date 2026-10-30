//! Source14.2.2/14.2.3 damage projected by every sensor/seeker consumer.
//! Per-hit timing, >= for14.2.3, repeat and whole-paragraph exemption are
//! explicit RTS interpretations in signature-damage-2026.json, not extra cells.
use super::damage_runtime::{Hit, RuleProfile};
use crate::db::WeaponKind;
use crate::types::PlatformKind;
use crate::world::Entity;
use crate::{EntityId, EventKind, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub fn tables() -> &'static Value {
    static TABLES: OnceLock<Value> = OnceLock::new();
    TABLES.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../data/rules/signature-damage-2026.json"
        ))
        .expect("verified signature damage rules and explicit RTS policy")
    })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub units: BTreeMap<EntityId, UnitDamage>,
    pub seen_hits: BTreeSet<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UnitDamage {
    pub class: String,
    pub radar: Option<RadarDamage>,
    pub acoustic: Option<AcousticDamage>,
    pub pending: BTreeMap<String, String>,
    pub hits: Vec<HitEvidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadarDamage {
    pub fitted_signature: usize,
    pub effective_signature: usize,
    pub physical_size: String,
    pub size_cap: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcousticDamage {
    pub fitted_noise: usize,
    pub fitted_anechoic: bool,
    pub effective_noise: usize,
    pub effective_anechoic: bool,
}
/// Referee evidence survives event-ring truncation and restores unchanged.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HitEvidence {
    pub identity: String,
    pub tick: i64,
    pub weapon: String,
    pub original_dp: f64,
    pub resolved_dp: f64,
    pub threshold_dp: f64,
    pub underwater: bool,
    pub underwater_class: Option<String>,
}
fn active(w: &World) -> bool {
    w.rules.profile == RuleProfile::Rts2026Full
}
fn real_hull(w: &World, e: &Entity) -> bool {
    matches!(e.kind, PlatformKind::Ship | PlatformKind::Sub)
        && super::ew2026::object(w, e.id).is_none()
}
fn radar_baseline(w: &World, e: &Entity, size: Option<&str>) -> Result<RadarDamage, String> {
    let profile = w
        .rules
        .detection
        .as_ref()
        .and_then(|s| s.config.targets.get(&e.class))
        .filter(|p| !p.source.trim().is_empty());
    let ops = super::operations::tables();
    let physical = size
        .filter(|s| !s.is_empty())
        .or_else(|| ops["radar"]["physical_sizes"][&e.class].as_str())
        .map(str::to_string)
        .or_else(|| {
            profile
                .filter(|p| p.size <= 6)
                .map(|p| ((b'A' + p.size as u8) as char).to_string())
        })
        .ok_or("14.2.2 original physical Size source missing")?;
    let cap = tables()["radar_size_cap"][&physical]
        .as_u64()
        .ok_or("14.2.2 physical Size is outside verified A-G cap table")? as usize;
    // Explicit source/author profile has the same priority as native radar.
    let signature = profile
        .and_then(|p| p.radar_signature)
        .or_else(|| {
            ops["radar"]["ship_signatures"][&e.class]
                .as_u64()
                .map(|n| n as usize)
        })
        .filter(|n| *n <= 4)
        .ok_or("14.2.2 original radar Signature source missing/invalid")?;
    if signature < cap {
        return Err(
            "14.2.2 original radar Signature exceeds physical Size; source conflict".into(),
        );
    }
    Ok(RadarDamage {
        fitted_signature: signature,
        effective_signature: signature,
        physical_size: physical,
        size_cap: cap,
    })
}
fn acoustic_baseline(w: &World, e: &Entity) -> Result<AcousticDamage, String> {
    let detection_profile = w
        .rules
        .detection
        .as_ref()
        .and_then(|s| s.config.targets.get(&e.class))
        .filter(|p| !p.source.trim().is_empty());
    let detection = detection_profile.and_then(|p| p.noise.map(|n| (n, p.anechoic)));
    let torpedo = w
        .rules
        .torpedoes
        .as_ref()
        .and_then(|s| s.config.targets.get(&e.class))
        .filter(|p| !p.source.trim().is_empty())
        .map(|p| (p.noise, p.anechoic));
    if detection.zip(torpedo).is_some_and(|(a, b)| a != b)
        || detection_profile
            .zip(torpedo)
            .is_some_and(|(p, (_, coating))| p.anechoic != coating)
    {
        return Err("14.2.3 fitted detection/torpedo acoustic source profiles conflict".into());
    }
    let (noise, coating) = detection
        .or(torpedo)
        .filter(|(n, _)| *n <= 4)
        .ok_or("14.2.3 original fitted noise/coating source missing/invalid")?;
    Ok(AcousticDamage {
        fitted_noise: noise,
        fitted_anechoic: coating,
        effective_noise: noise,
        effective_anechoic: coating,
    })
}
fn underwater_class(hit: &Hit) -> Result<&'static str, String> {
    if let Some(cause) = &hit.asw {
        return Ok(match cause {
            super::damage_runtime::AswImpact::DepthChargeSelfBlast { .. } => "source_depth_charge_own_underwater_blast_8.5.4.3",
            super::damage_runtime::AswImpact::ProjectorContact { .. } => "source_committed_projector_contact_charge_8.5.3",
            super::damage_runtime::AswImpact::Hydrostatic { .. } => "source_committed_hydrostatic_asw_pattern_8.5",
            super::damage_runtime::AswImpact::ClusterContact { .. } => "source_committed_air_asw_cluster_contact_8.5.4.4",
        });
    }
    if hit.projector.is_some() {
        return Ok("source_committed_hydrostatic_asw_pattern_8.5");
    }
    if hit.mine.is_some() {
        return Ok("source_mine_13.6");
    }
    if matches!(hit.weapon.kind, WeaponKind::Torpedo | WeaponKind::Lwt) {
        return Ok("torpedo_14.1.7");
    }
    Err("14.2.3 underwater attack provenance pending: bool alone does not verify depth-charge/ASW payload/nuclear class".into())
}
fn evidence_detail(e: &HitEvidence) -> Value {
    json!({"hit":e.identity,"tick":e.tick,"weapon":e.weapon,"original_dp":e.original_dp,
        "resolved_dp":e.resolved_dp,"threshold_dp":e.threshold_dp,"underwater":e.underwater,
        "underwater_class":e.underwater_class,"source":"English2026 PDF132/14-6 §§14.2.2-14.2.3",
        "policy":tables()["rts_policy"]["id"]})
}
/// Called only after actual primary DP commitment, never by commands or AI.
pub(crate) fn commit(
    w: &mut World,
    before: &Entity,
    physical_size: Option<&str>,
    hit: &Hit,
    dp: f64,
) {
    if !active(w) || hit.secondary || hit.navigation.is_some() || !real_hull(w, before) || !dp.is_finite() || dp <= 0. {
        return;
    }
    let underwater=hit.underwater.then(||underwater_class(hit).ok().map(str::to_string)).flatten();
    let evidence=HitEvidence {identity:hit.identity.clone(),tick:hit.tick,weapon:hit.weapon.id.clone(),original_dp:before.hp_max,resolved_dp:dp,threshold_dp:0.,underwater:hit.underwater,underwater_class:underwater};
    commit_evidence(w,before,physical_size,evidence);
}
/// Only the nuclear resolver can create this non-deserializable proof after
/// committing actual primary DP. No invented mine/torpedo provenance.
pub(crate) fn commit_nuclear(w:&mut World,before:&Entity,hit:&super::nuclear2026::PrimaryDamageEvidence){
    let underwater=hit.medium()==super::nuclear2026::Medium::Subsurface;
    commit_evidence(w,before,None,HitEvidence {identity:hit.identity().into(),tick:hit.tick(),weapon:hit.weapon().into(),original_dp:before.hp_max,resolved_dp:hit.dp(),threshold_dp:0.,underwater,underwater_class:underwater.then(||"source_nuclear_14.1.8".into())});
}
fn commit_evidence(w:&mut World,before:&Entity,physical_size:Option<&str>,mut evidence:HitEvidence){
    let dp=evidence.resolved_dp;
    if !active(w)||!real_hull(w,before)||!dp.is_finite()||dp<=0. {return;}
    let threshold=before.hp_max*tables()["threshold_fraction_original_dp"].as_f64().unwrap();
    if !before.hp_max.is_finite()||before.hp_max<=0.||dp<threshold{return;}
    evidence.threshold_dp=threshold;
    let mut state=w.rules.signature_damage.take().unwrap_or_default();
    if !state.seen_hits.insert(format!("{}:{}",before.id.0,evidence.identity)){w.rules.signature_damage=Some(state);return;}
    let u=state.units.entry(before.id).or_insert_with(||UnitDamage {class:before.class.clone(),..Default::default()});
    let mut events: Vec<(&str, Value)> = vec![];
    if before.kind == PlatformKind::Ship && !u.pending.contains_key("radar") {
        match radar_baseline(w, before, physical_size) {
            Ok(base) => {
                let r = u.radar.get_or_insert_with(|| base.clone());
                if r.fitted_signature != base.fitted_signature
                    || r.physical_size != base.physical_size
                {
                    u.pending.insert(
                        "radar".into(),
                        "14.2.2 immutable fitted radar/Size source changed after damage".into(),
                    );
                } else if r.effective_signature > r.size_cap {
                    let old = r.effective_signature;
                    r.effective_signature = (old - 1).max(r.size_cap);
                    let mut d = evidence_detail(&evidence);
                    d["before"] = json!(old);
                    d["after"] = json!(r.effective_signature);
                    d["size_cap"] = json!(r.size_cap);
                    d["physical_size"] = json!(r.physical_size);
                    events.push(("14.2.2.signature", d));
                }
            }
            Err(reason) => {
                u.pending.insert("radar".into(), reason);
            }
        }
    }
    if evidence.underwater && !u.pending.contains_key("acoustic") {
        match evidence.underwater_class.as_ref().ok_or_else(||"14.2.3 underwater primary provenance pending".to_string()).and_then(|_| acoustic_baseline(w, before)) {
            Ok(base) => {
                let a = u.acoustic.get_or_insert_with(|| base.clone());
                if a.fitted_noise != base.fitted_noise || a.fitted_anechoic != base.fitted_anechoic
                {
                    u.pending.insert(
                        "acoustic".into(),
                        "14.2.3 immutable fitted acoustic source changed after damage".into(),
                    );
                } else if a.fitted_noise
                    > tables()["noise_code_indices"]["Noisy"].as_u64().unwrap() as usize
                    && a.effective_noise == a.fitted_noise
                {
                    let old = a.effective_noise;
                    let coating = a.effective_anechoic;
                    a.effective_noise =
                        tables()["noise_code_indices"]["Noisy"].as_u64().unwrap() as usize;
                    a.effective_anechoic = false;
                    let mut d = evidence_detail(&evidence);
                    d["before"] = json!(old);
                    d["after"] = json!(a.effective_noise);
                    d["anechoic_before"] = json!(coating);
                    d["anechoic_after"] = json!(false);
                    events.push(("14.2.3.acoustic", d));
                }
            }
            Err(reason) => {
                u.pending.insert("acoustic".into(), reason);
            }
        }
    }
    if !u.pending.is_empty() {
        let mut d = evidence_detail(&evidence);
        d["branches"] = json!(u.pending);
        events.push(("14.2.signature_pending", d));
    }
    u.hits.push(evidence);
    w.rules.signature_damage = Some(state);
    for (rule, detail) in events {
        w.emit(
            EventKind::RulesEvent {
                domain: "signature_damage2026".into(),
                rule: rule.into(),
                side: Some(before.side),
                unit: Some(before.id),
                contact: None,
                detail: detail.to_string(),
            },
            Visibility::Referee,
        );
    }
}
fn unit<'a>(w: &'a World, id: EntityId, branch: &str) -> Result<Option<&'a UnitDamage>, String> {
    if !active(w) {
        return Ok(None);
    }
    let Some(e) = w.entity(id) else {
        return Ok(None);
    };
    if !real_hull(w, e) {
        return Ok(None);
    }
    let Some(u) = w
        .rules
        .signature_damage
        .as_ref()
        .and_then(|s| s.units.get(&id))
    else {
        return Ok(None);
    };
    if e.class != u.class {
        return Err("14.2 damaged hull immutable class changed".into());
    }
    if let Some(reason) = u.pending.get(branch) {
        return Err(reason.clone());
    }
    Ok(Some(u))
}
pub fn project_radar(w: &World, id: EntityId, fitted_signature: usize) -> Result<usize, String> {
    let Some(u) = unit(w, id, "radar")? else {
        return Ok(fitted_signature);
    };
    let Some(r) = &u.radar else {
        return Ok(fitted_signature);
    };
    if fitted_signature != r.fitted_signature {
        return Err("14.2.2 consumer fitted radar Signature differs from committed source".into());
    }
    Ok(r.effective_signature)
}
pub fn project_noise(w: &World, id: EntityId, fitted_noise: usize) -> Result<usize, String> {
    let Some(u) = unit(w, id, "acoustic")? else {
        return Ok(fitted_noise);
    };
    let Some(a) = &u.acoustic else {
        return Ok(fitted_noise);
    };
    if fitted_noise != a.fitted_noise {
        return Err("14.2.3 consumer fitted noise differs from committed source".into());
    }
    Ok(a.effective_noise)
}
pub fn project_acoustic(
    w: &World,
    id: EntityId,
    fitted_noise: Option<usize>,
    fitted_anechoic: bool,
) -> Result<(Option<usize>, bool), String> {
    let Some(u) = unit(w, id, "acoustic")? else {
        return Ok((fitted_noise, fitted_anechoic));
    };
    let Some(a) = &u.acoustic else {
        return Ok((fitted_noise, fitted_anechoic));
    };
    if fitted_noise != Some(a.fitted_noise) || fitted_anechoic != a.fitted_anechoic {
        return Err("14.2.3 consumer fitted noise/coating differs from committed source".into());
    }
    Ok((Some(a.effective_noise), a.effective_anechoic))
}
/// Active echo strength needs coating, not passive noise. A missing optional
/// detection noise cannot erase a complete, independently verified torpedo
/// baseline; a supplied noise still must agree with that baseline.
pub fn project_coating(
    w: &World,
    id: EntityId,
    fitted_noise: Option<usize>,
    fitted_anechoic: bool,
) -> Result<bool, String> {
    let Some(u) = unit(w, id, "acoustic")? else {
        return Ok(fitted_anechoic);
    };
    let Some(a) = &u.acoustic else {
        return Ok(fitted_anechoic);
    };
    if fitted_noise.is_some_and(|n| n != a.fitted_noise) || fitted_anechoic != a.fitted_anechoic {
        return Err("14.2.3 consumer fitted coating/noise differs from committed source".into());
    }
    Ok(a.effective_anechoic)
}
/// Call only while constructing the owner's unit list. No hit/weapon/shooter,
/// foreign IDs, full evidence or failed-scan truth appears in this whitelist.
pub fn own_view(w: &World, id: EntityId) -> Option<Value> {
    if !active(w) || !w.entity(id).is_some_and(|e| real_hull(w, e)) {
        return None;
    }
    let u = w.rules.signature_damage.as_ref()?.units.get(&id)?;
    let radar = u.radar.as_ref().filter(|r| r.effective_signature != r.fitted_signature)
        .map(|r| json!({"fitted":r.fitted_signature,"effective":r.effective_signature,"size_cap":r.size_cap}));
    let acoustic = u.acoustic.as_ref().filter(|a| a.effective_noise != a.fitted_noise || a.effective_anechoic != a.fitted_anechoic)
        .map(|a| json!({"fitted_noise":a.fitted_noise,"effective_noise":a.effective_noise,
            "anechoic_benefit":a.effective_anechoic,"coating_benefit_lost":a.fitted_anechoic && !a.effective_anechoic}));
    if radar.is_none() && acoustic.is_none() && u.pending.is_empty() {
        return None;
    }
    Some(
        json!({"source":"English2026 14.2.2/14.2.3","policy":tables()["rts_policy"]["id"],
        "radar":radar,"acoustic":acoustic,"pending":u.pending}),
    )
}

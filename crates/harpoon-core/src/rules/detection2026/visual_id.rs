//! Optional5.8.9 identity rights consume an actual native Visual success.
//! Measurement/fusion remain in the normal sensor pass; no new contacts or FCS.
use super::{Receiver, ScanDecision};
use crate::types::{DayNight, PlatformKind};
use crate::{EntityId, Rng, Side, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub fn tables() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/visual-identification-2026.json"
        ))
        .expect("visually verified optional identification source data")
    })
}
/// Exact named scenario body and class, not a class-prefix nationality guess.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityMetadata {
    pub source: String,
    pub class_id: String,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub platform_type: Option<String>,
    #[serde(default)]
    pub nationality: Option<String>,
    #[serde(default)]
    pub class_label: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub enabled: bool,
    pub metadata: BTreeMap<String, IdentityMetadata>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Labels {
    pub size: Option<String>,
    pub platform_type: Option<String>,
    pub nationality: Option<String>,
    pub class_label: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trial {
    pub epoch: i64,
    pub at: i64,
    pub detection_d100: Option<u8>,
    pub original_d10: u8,
    pub modified_d10: Option<i32>,
    pub range_nm: f64,
    pub visibility_percent: Option<u8>,
    pub daylight: bool,
    pub horizon_kyd: Option<f64>,
    pub hull_up: Option<bool>,
    pub source_level: Option<u8>,
    pub earned_level: u8,
    pub labels: Labels,
    pub pending_reason: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdentityTrack {
    pub observer: EntityId,
    pub sensor: String,
    pub observer_side: Side,
    pub observer_class: String,
    pub target: EntityId,
    pub target_class: String,
    pub target_side: Side,
    pub first_seen: i64,
    pub initial_epoch: i64,
    pub contact_alias: Option<crate::ContactId>,
    pub continuity_pending: Option<String>,
    pub last_seen: i64,
    pub earned_at: i64,
    pub earned_level: u8,
    pub labels: Labels,
    pub trials: BTreeMap<i64, Trial>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub config: Configuration,
    pub tracks: BTreeMap<String, IdentityTrack>,
}
pub fn initialize(config: Option<&Configuration>) -> Result<Option<State>, String> {
    let Some(config) = config.filter(|c| c.enabled) else {
        return Ok(None);
    };
    if config.source.trim().is_empty() {
        return Err("5.8.9 optional identification requires explicit source".into());
    }
    for (name, m) in &config.metadata {
        if name.trim().is_empty()
            || m.source.trim().is_empty()
            || m.class_id.trim().is_empty()
            || [&m.size, &m.platform_type, &m.nationality, &m.class_label]
                .iter()
                .any(|x| x.as_ref().is_some_and(|s| s.trim().is_empty()))
        {
            return Err("5.8.9 identity labels require an exact named source/class and nonempty supplied labels".into());
        }
    }
    Ok(Some(State {
        config: config.clone(),
        ..Default::default()
    }))
}
fn fail(trial: &mut Trial, reason: &str) {
    trial.pending_reason.get_or_insert_with(|| reason.into());
}
fn label_level(m: &IdentityMetadata, level: u8) -> (u8, Labels) {
    let mut labels = Labels::default();
    if level == 0 {
        return (0, labels);
    }
    let Some(size) = m.size.as_ref() else {
        return (0, labels);
    };
    labels.size = Some(size.clone());
    if level == 1 {
        return (1, labels);
    }
    let Some(kind) = m.platform_type.as_ref() else {
        return (1, labels);
    };
    labels.platform_type = Some(kind.clone());
    if level == 2 {
        return (2, labels);
    }
    let Some(nation) = m.nationality.as_ref() else {
        return (2, labels);
    };
    labels.nationality = Some(nation.clone());
    if level == 3 {
        return (3, labels);
    }
    let Some(class) = m.class_label.as_ref() else {
        return (3, labels);
    };
    labels.class_label = Some(class.clone());
    (4, labels)
}
fn trial(
    w: &World,
    state: &super::State,
    config: &Configuration,
    observer: EntityId,
    sensor: &str,
    target: EntityId,
    scan: &ScanDecision,
    epoch: i64,
) -> Trial {
    let o = w.entity(observer).unwrap();
    let t = w.entity(target).unwrap();
    let key = format!(
        "{}:{}:{sensor}:{}",
        tables()["rng_key_prefix"].as_str().unwrap(),
        observer.0,
        target.0
    );
    let mut rng = Rng::new(w.seed).derive(Rng::hash_tag(epoch, 0, &key));
    let mut out = Trial {
        epoch,
        at: w.tick,
        detection_d100: scan.trace["d100"]
            .as_u64()
            .filter(|d| (1..=100).contains(d))
            .map(|d| d as u8),
        original_d10: 1 + (rng.next_f64() * 10.) as u8,
        modified_d10: None,
        range_nm: o.pos.range_to(t.pos),
        visibility_percent: state
            .config
            .environment
            .as_ref()
            .map(|e| e.visibility_percent),
        daylight: w.day_night == DayNight::Day,
        horizon_kyd: None,
        hull_up: None,
        source_level: None,
        earned_level: 0,
        labels: Labels::default(),
        pending_reason: None,
    };
    if o.kind != PlatformKind::Ship {
        fail(
            &mut out,
            "5.8.9 airborne/land observer horizon source not bound in this native surface scope",
        );
        return out;
    }
    let lit = w.day_night == DayNight::Night && state.config.environment.as_ref()
        .and_then(|env|super::super::surface2026::gunnery::illumination_with_environment(w,observer,target,env)) == Some(false);
    if w.day_night != DayNight::Day && !lit {
        fail(&mut out,"5.8.9 night identification needs actual qualified illumination or EOIR; EOIR remains pending");
        return out;
    }
    let Some(profile) = state
        .config
        .targets
        .get(&o.class)
        .filter(|p| !p.source.trim().is_empty())
    else {
        fail(&mut out, "5.8.9 actual observer source size/horizon absent");
        return out;
    };
    let size = match profile.size {
        0..=4 => profile.size,
        5 | 6 => 5,
        7 => 6,
        _ => {
            fail(
                &mut out,
                "5.8.9 actual observer physical size outside source horizon table",
            );
            return out;
        }
    };
    let Some(horizon) = tables()["surface_horizon_kyd"][size].as_f64() else {
        fail(&mut out, "5.8.9 source horizon cell absent");
        return out;
    };
    let kyd = out.range_nm * tables()["conversion"]["yards_per_nm"].as_f64().unwrap()
        / tables()["conversion"]["yards_per_kyd"].as_f64().unwrap();
    if !out.range_nm.is_finite() || !o.pos.x.is_finite() || !o.pos.y.is_finite() {
        fail(&mut out, "5.8.9 native geometry is nonfinite");
        return out;
    }
    out.horizon_kyd = Some(horizon);
    out.hull_up = Some(kyd <= horizon);
    let Some(d100) = out.detection_d100 else {
        fail(
            &mut out,
            "5.8.9 actual successful lookout original detectionD100 missing",
        );
        return out;
    };
    let Some(visibility) = out.visibility_percent else {
        fail(&mut out, "5.8.9 actual explicit visibility absent");
        return out;
    };
    let column = tables()["detection_roll_upper"]
        .as_array()
        .unwrap()
        .iter()
        .position(|n| u64::from(d100) <= n.as_u64().unwrap())
        .unwrap();
    let mut modifier = tables()["detection_modifiers"][column].as_i64().unwrap() as i32;
    let close = &tables()["close_contact"];
    if out.range_nm < close["range_nm_exclusive"].as_f64().unwrap()
        && kyd < close["range_kyd_exclusive"].as_f64().unwrap()
        && u64::from(visibility) >= close["minimum_visibility_percent"].as_u64().unwrap()
    {
        modifier += close["modifier"].as_i64().unwrap() as i32;
    }
    if lit {
        modifier += tables()["night_illuminated_modifier"].as_i64().unwrap() as i32; // PDF68 priority over visibility.
    } else if let Some(env) = tables()["environment_visibility"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| u64::from(visibility) <= m["maximum_percent"].as_u64().unwrap())
    {
        modifier += env["modifier"].as_i64().unwrap() as i32;
    }
    let modified = i32::from(out.original_d10) + modifier;
    out.modified_d10 = Some(modified);
    if !(1..=10).contains(&modified) {
        fail(
            &mut out,
            "5.8.9 modifiedD10 outside printed1..10; original retained, no guessed clamp",
        );
        return out;
    }
    if out.hull_up == Some(true) && modified == 4 {
        fail(&mut out,"5.8.9 English worked example modified4 says Ship but HullUp table4 says Size; specific pending");
        return out;
    }
    let level = tables()[if out.hull_up == Some(true) {
        "hull_up"
    } else {
        "hull_down"
    }][(modified - 1) as usize]
        .as_u64()
        .unwrap() as u8;
    out.source_level = Some(level);
    if level == 0 {
        return out;
    }
    let unique = w.entities.values().filter(|e| e.name == t.name).count() == 1;
    let Some(metadata) = config
        .metadata
        .get(&t.name)
        .filter(|m| unique && m.class_id == t.class && !m.source.trim().is_empty())
    else {
        fail(&mut out,"5.8.9 earned higher identity needs exact named/class source labels; no true name or nationality-prefix fallback");
        return out;
    };
    let (earned, labels) = label_level(metadata, level);
    out.earned_level = earned;
    out.labels = labels;
    if earned < level {
        fail(&mut out,"5.8.9 source labels needed by earned table rights are incomplete; undisclosed identity stays unknown");
    }
    out
}
/// Called only after a normal native evaluation and before report projection.
/// It never creates measurements, contacts, a second scan or a new sensor kind.
pub fn apply(
    w: &World,
    state: &mut super::State,
    observer: EntityId,
    sensor: &str,
    target: EntityId,
    scan: &mut ScanDecision,
) {
    if !scan.detected
        || !state
            .visual_identification
            .as_ref()
            .is_some_and(|s| s.config.enabled)
    {
        return;
    }
    let Some(o) = w.entity(observer) else {
        return;
    };
    let Some(t) = w.entity(target) else {
        return;
    };
    if t.kind != PlatformKind::Ship
        || !matches!(
            state.config.bindings.get(&format!("{}:{sensor}", o.class)),
            Some(Receiver::Visual { .. })
        )
    {
        return;
    }
    // This receiver cannot inherit Basic's unconditional true-class projection.
    // Other actual receivers are fused later and retain their independent rights.
    scan.identify_class = false;
    scan.class_label = None;
    let epoch = w
        .tick
        .div_euclid(tables()["epoch_seconds"].as_i64().unwrap());
    let key = format!("{}:{}:{sensor}:{}", o.side.index(), observer.0, target.0);
    let config = state.visual_identification.as_ref().unwrap().config.clone();
    let cached = state
        .visual_identification
        .as_ref()
        .unwrap()
        .tracks
        .get(&key)
        .cloned();
    let alias = w.sides[o.side.index()]
        .contacts
        .values()
        .find(|c| c.about == target && !c.stale)
        .map(|c| (c.id, c.first_tick, c.last_update_tick));
    let mut continuity = cached.as_ref().and_then(|x| x.continuity_pending.clone());
    if let Some(old) = &cached {
        let reason = if old.observer != observer
            || old.sensor != sensor
            || old.observer_side != o.side
            || old.observer_class != o.class
            || old.target != target
            || old.target_class != t.class
            || old.target_side != t.side
        {
            Some("physical observer/owner/target identity binding changed")
        } else if epoch != old.initial_epoch {
            Some("cross-epoch re-identification source policy pending; old private body identity grants no new class")
        } else if w.tick < old.last_seen || w.tick - old.last_seen > 1 {
            Some("native Visual continuity lost; reacquisition requires a separately sourced identity episode")
        } else if match (old.contact_alias, alias) {
            (Some(expected), Some((now, _, _))) => expected != now,
            // First native scan precedes normal contact projection. Bind only
            // the contact actually born with that first measured report.
            (None, Some((_, first, last))) => first != old.first_seen || last < old.first_seen,
            (None, None) => w.tick != old.first_seen,
            (Some(_), None) => true,
        } {
            Some("original visible ContactID unavailable or changed; no hidden-EntityId knowledge transfer")
        } else if let Some(first) = old.trials.get(&old.initial_epoch) {
            let range = o.pos.range_to(t.pos);
            let kyd = range * tables()["conversion"]["yards_per_nm"].as_f64().unwrap()
                / tables()["conversion"]["yards_per_kyd"].as_f64().unwrap();
            let visibility = state
                .config
                .environment
                .as_ref()
                .map(|e| e.visibility_percent);
            let close = |nm: f64, vis: Option<u8>| {
                nm < tables()["close_contact"]["range_nm_exclusive"]
                    .as_f64()
                    .unwrap()
                    && nm * tables()["conversion"]["yards_per_nm"].as_f64().unwrap()
                        / tables()["conversion"]["yards_per_kyd"].as_f64().unwrap()
                        < tables()["close_contact"]["range_kyd_exclusive"]
                            .as_f64()
                            .unwrap()
                    && vis.is_some_and(|v| {
                        u64::from(v)
                            >= tables()["close_contact"]["minimum_visibility_percent"]
                                .as_u64()
                                .unwrap()
                    })
            };
            if visibility != first.visibility_percent
                || (w.day_night == DayNight::Day) != first.daylight
                || first
                    .horizon_kyd
                    .is_some_and(|h| Some(kyd <= h) != first.hull_up)
                || close(range, visibility) != close(first.range_nm, first.visibility_percent)
            {
                Some("identification visibility/hull/close-contact condition changed; original dice retained, new identity qualification pending")
            } else {
                None
            }
        } else {
            Some("original native identification episode snapshot missing")
        };
        if let Some(reason) = reason {
            continuity.get_or_insert_with(|| reason.into());
        }
    }
    let original = cached
        .as_ref()
        .and_then(|x| x.trials.get(&epoch))
        .cloned()
        .unwrap_or_else(|| trial(w, state, &config, observer, sensor, target, scan, epoch));
    let s = state.visual_identification.as_mut().unwrap();
    let track = s.tracks.entry(key).or_insert_with(|| IdentityTrack {
        observer,
        sensor: sensor.into(),
        observer_side: o.side,
        observer_class: o.class.clone(),
        target,
        target_class: t.class.clone(),
        target_side: t.side,
        first_seen: w.tick,
        initial_epoch: epoch,
        contact_alias: alias.map(|a| a.0),
        continuity_pending: None,
        last_seen: w.tick,
        earned_at: w.tick,
        earned_level: 0,
        labels: Labels::default(),
        trials: BTreeMap::new(),
    });
    let reused = track.trials.contains_key(&epoch);
    track
        .trials
        .entry(epoch)
        .or_insert_with(|| original.clone());
    track.last_seen = w.tick;
    track.continuity_pending = continuity.clone();
    if continuity.is_none() {
        if track.contact_alias.is_none() {
            track.contact_alias = alias.map(|a| a.0);
        }
        if original.earned_level > track.earned_level {
            track.earned_level = original.earned_level;
            track.labels = original.labels.clone();
            track.earned_at = original.at;
        }
        if original.pending_reason.is_none() && track.earned_level == 4 {
            scan.identify_class = true;
            scan.class_label = track.labels.class_label.clone();
        }
    }
    scan.trace["visual_identification"] = json!({"rule":"5.8.9","status":if continuity.is_some()||original.pending_reason.is_some(){"source_pending"}else{"resolved"},"reason":continuity,"original":original,"reused":reused,"earned_level":track.earned_level,"source":"English2026 PDF67/68; same native epoch/original visible alias only, no hidden-body knowledge transfer"});
}
pub fn own_view(w: &World, side: Side, unit: EntityId) -> Value {
    let Some(o) = w.entity(unit).filter(|o| o.side == side) else {
        return Value::Null;
    };
    let Some(state) = w
        .rules
        .detection
        .as_ref()
        .and_then(|s| s.visual_identification.as_ref())
        .filter(|s| s.config.enabled)
    else {
        return Value::Null;
    };
    let entries:Vec<_>=state.tracks.values().filter(|x|x.observer==unit&&x.observer_side==side&&x.observer_class==o.class).filter_map(|x|{
        let c=w.sides[side.index()].contacts.values().find(|c|c.about==x.target&&!c.stale && match x.contact_alias {Some(cid)=>c.id==cid,None=>c.first_tick==x.first_seen&&c.last_update_tick>=x.first_seen})?;
        let latest=x.trials.values().next_back()?;
        let pending=x.continuity_pending.is_some();
        let empty=Labels::default();let labels=if pending{&empty}else{&x.labels};
        Some(json!({"contact":c.id,"sensor":x.sensor,"last_native_seen_at":x.last_seen,"identified_at":x.earned_at,"knowledge":tables()["knowledge_levels"][if pending{0}else{usize::from(x.earned_level)}],"size":labels.size,"type":labels.platform_type,"nationality":labels.nationality,"class":labels.class_label,"status":if pending||latest.pending_reason.is_some(){"source_pending"}else{"resolved"}}))
    }).collect();
    json!({"enabled":true,"scope":"native_surface_visual_identity","contacts":entries})
}

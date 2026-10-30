//! Source5.7.2 prelaid-field wide MAD. Ordinary scans share only earned-fix clocks.
use super::{Receiver, ScanDecision, SensorReport};
use crate::db::SensorKind;
use crate::types::PlatformKind;
use crate::{EntityId, EventKind, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub fields: BTreeMap<String, Field>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialTurnPolicy {
    #[default]
    Pending,
    FullTurnsOnly,
    AnyPresence,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepthTiePolicy {
    #[default]
    Pending,
    Shallower,
    Deeper,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub source: String,
    pub observer: String,
    pub sensor: String,
    pub center: Vec2,
    pub course_deg: f64,
    pub columns: u16,
    pub rows: u16,
    pub spacing_nm: f64,
    pub prelaid: bool,
    pub enabled: bool,
    #[serde(default)]
    pub partial_turn_policy: PartialTurnPolicy,
    #[serde(default)]
    pub depth_tie_policy: DepthTiePolicy,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundField {
    pub config: Field,
    pub observer: EntityId,
    pub class: String,
    pub side: Side,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Exposure {
    pub target: Option<EntityId>,
    pub field: String,
    pub it: i64,
    pub turn_seconds: BTreeMap<i64, u16>,
    pub depth_seconds: BTreeMap<usize, u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MadFix {
    pub report: SensorReport,
    pub usable_turn: i64,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixHistory {
    pub latest: MadFix,
    pub previous: Option<MadFix>,
    pub observer_class: String,
    pub side: Side,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trace {
    pub actor: EntityId,
    pub rule: String,
    pub detail: Value,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub fields: BTreeMap<String, BoundField>,
    pub exposures: BTreeMap<String, Exposure>,
    pub completed: BTreeSet<String>,
    pub fixes: BTreeMap<String, FixHistory>,
    pub last_sample_at: Option<i64>,
    pub traces: Vec<Trace>,
}
fn tac_seconds() -> i64 {
    super::tables()["mad"]["wide_execution"]["tactical_turn_seconds"]
        .as_i64()
        .unwrap()
}
fn it_seconds() -> i64 {
    super::tables()["mad"]["wide_execution"]["intermediate_turn_seconds"]
        .as_i64()
        .unwrap()
}
fn low_altitude(altitude: f64) -> bool {
    let row = &super::tables()["mad"]["wide_execution"]["low_altitude_m"];
    altitude >= row[0].as_f64().unwrap() && altitude <= row[1].as_f64().unwrap()
}
fn area(f: &Field) -> f64 {
    (f64::from(f.columns) + 1.) * (f64::from(f.rows) + 1.) * f.spacing_nm * f.spacing_nm
}
fn inside(f: &Field, p: Vec2) -> bool {
    let a = f.course_deg.to_radians();
    let dx = p.x - f.center.x;
    let dy = p.y - f.center.y;
    let along = dx * a.sin() + dy * a.cos();
    let across = dx * a.cos() - dy * a.sin();
    along.abs() <= (f64::from(f.columns) + 1.) * f.spacing_nm / 2. + 1e-9
        && across.abs() <= (f64::from(f.rows) + 1.) * f.spacing_nm / 2. + 1e-9
}
fn receiver<'a>(s: &'a super::State, class: &str, sensor: &str) -> Option<&'a Receiver> {
    s.config.bindings.get(&format!("{class}:{sensor}"))
}
fn fitted(w: &World, s: &super::State, b: &BoundField) -> bool {
    w.entity(b.observer).is_some_and(|e| e.alive && e.kind==PlatformKind::Aircraft
        && e.class==b.class && e.side==b.side && e.sensors.iter().any(|i|i.sensor==b.config.sensor)
        && w.db.sensors.get(&b.config.sensor).is_some_and(|d|d.kind==SensorKind::Magnetic)
        && matches!(receiver(s,&e.class,&b.config.sensor),Some(Receiver::Mad{source,western:false,..}) if !source.trim().is_empty())
        && !crate::sensors::sensor_damaged(e,&b.config.sensor)
        && crate::rules::expanded_damage2026::sensor_operational(w,e.id,&b.config.sensor).unwrap_or(false)
        && crate::rules::unmanned2026::detection_allowed(w,e.id))
}
fn monitoring(w: &World, s: &super::State, b: &BoundField) -> bool {
    fitted(w, s, b)
        && w.entity(b.observer).is_some_and(|e| {
            b.config.enabled
                && low_altitude(e.altitude_m)
                && e.speed_kts > 0.
                && e.speed_kts.is_finite()
                && inside(&b.config, e.pos)
                && crate::geom::angle_diff(e.course_deg, e.ordered_course_deg).abs() <= 0.1
                && (e.altitude_m - e.ordered_altitude_m).abs() <= 1.
        })
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    let Some(s) = w.rules.detection.as_ref() else {
        return Ok(());
    };
    let mut fields = BTreeMap::new();
    let mut observers = BTreeSet::new();
    if let Some(config) = &s.config.wide_mad {
        if config.source.trim().is_empty() {
            return Err("5.7.2 wide MAD configuration requires source".into());
        }
        for (id, f) in &config.fields {
            if id.trim().is_empty()
                || f.source.trim().is_empty()
                || !f.prelaid
                || f.columns == 0
                || f.rows == 0
                || !f.spacing_nm.is_finite()
                || f.spacing_nm <= 0.
                || !f.course_deg.is_finite()
                || !f.center.x.is_finite()
                || !f.center.y.is_finite()
            {
                return Err(format!(
                    "5.7.2 {id}: explicit sourced prelaid field geometry required"
                ));
            }
            super::wide_mad_probability(area(f), 1, 10)?;
            let units: Vec<_> = w
                .entities
                .values()
                .filter(|e| e.name == f.observer)
                .collect();
            if units.len() != 1 {
                return Err(format!(
                    "5.7.2 {id}: actual observer name must resolve uniquely"
                ));
            }
            let e = units[0];
            if !observers.insert(e.id) {
                return Err("5.7.2 multiple simultaneous fields for one observer require a separate source allocation policy".into());
            }
            let b = BoundField {
                config: f.clone(),
                observer: e.id,
                class: e.class.clone(),
                side: e.side,
            };
            if !fitted(w, s, &b) {
                return Err(format!("5.7.2 {id}: actual live installed available Magnetic/nonwestern Mad source fit required"));
            }
            fields.insert(id.clone(), b);
        }
    }
    w.rules.detection.as_mut().unwrap().wide_mad.fields = fields;
    Ok(())
}
pub fn control(w: &mut World, actor: EntityId, field: &str, enabled: bool) -> Result<(), String> {
    let s = w
        .rules
        .detection
        .as_ref()
        .ok_or("5.7.2 detection state absent")?;
    let b = s
        .wide_mad
        .fields
        .get(field)
        .ok_or("5.7.2 own configured prelaid field absent")?;
    if b.observer != actor || !fitted(w, s, b) {
        return Err("5.7.2 own actual qualified field observer required".into());
    }
    if b.config.enabled == enabled {
        return Ok(());
    }
    let s = w.rules.detection.as_mut().unwrap();
    s.wide_mad.fields.get_mut(field).unwrap().config.enabled = enabled;
    // Stopping cannot bank monitoring work for a later restart.
    s.wide_mad.exposures.retain(|_, x| x.field != field);
    Ok(())
}
fn trace(s: &mut super::State, actor: EntityId, rule: &str, detail: Value) {
    s.wide_mad.traces.push(Trace {
        actor,
        rule: rule.into(),
        detail,
    });
}
/// Called exactly once in the normal v4 sensor phase. No second sensor scan.
pub fn scan_world(w: &World, s: &mut super::State) -> Vec<(ScanDecision, SensorReport)> {
    let mut reports = Vec::new();
    if !super::active(w) || w.tick <= 0 || s.wide_mad.last_sample_at == Some(w.tick) {
        return reports;
    }
    s.wide_mad.last_sample_at = Some(w.tick);
    let it = (w.tick - 1).div_euclid(it_seconds());
    let tac = (w.tick - 1).div_euclid(tac_seconds());
    let fields: Vec<_> = s
        .wide_mad
        .fields
        .iter()
        .map(|(id, b)| (id.clone(), b.clone()))
        .collect();
    for (id, b) in &fields {
        if !monitoring(w, s, b) {
            continue;
        }
        for target in w.entities.values().filter(|e| {
            e.alive
                && e.kind == PlatformKind::Sub
                && e.side != b.side
                && e.depth_m > 0.
                && inside(&b.config, e.pos)
        }) {
            let Ok(depth) = super::depth_band(target.depth_m) else {
                continue;
            };
            let key = format!("{id}:{}:{it}", target.id.0);
            let x = s.wide_mad.exposures.entry(key).or_insert_with(|| Exposure {
                target: Some(target.id),
                field: id.clone(),
                it,
                ..Default::default()
            });
            *x.turn_seconds.entry(tac).or_default() += 1;
            *x.depth_seconds.entry(depth).or_default() += 1;
        }
    }
    if w.tick % it_seconds() != 0 {
        return reports;
    }
    let keys: Vec<_> = s
        .wide_mad
        .exposures
        .iter()
        .filter(|(_, x)| x.it <= it)
        .map(|(k, _)| k.clone())
        .collect();
    for key in keys {
        let x = s.wide_mad.exposures.remove(&key).unwrap();
        let Some(target) = x.target else { continue };
        let Some(b) = s.wide_mad.fields.get(&x.field).cloned() else {
            continue;
        };
        let identity = format!("5.7.2:{}:{}:{}:{}", x.field, b.observer.0, target.0, x.it);
        if !s.wide_mad.completed.insert(identity.clone()) {
            continue;
        }
        let partial = x
            .turn_seconds
            .values()
            .any(|n| i64::from(*n) != tac_seconds());
        let mut reason = None;
        let turns = match b.config.partial_turn_policy {
            PartialTurnPolicy::Pending if partial => {
                reason = Some("partial Tactical Turn exposure has no selected counting policy");
                0
            }
            PartialTurnPolicy::AnyPresence => x.turn_seconds.len(),
            _ => x
                .turn_seconds
                .values()
                .filter(|n| i64::from(**n) == tac_seconds())
                .count(),
        };
        let total: u32 = x.depth_seconds.values().sum();
        let max = x.depth_seconds.values().copied().max().unwrap_or(0);
        let winners: Vec<_> = x
            .depth_seconds
            .iter()
            .filter(|(_, n)| **n == max)
            .map(|(d, _)| *d)
            .collect();
        let depth = if max * 2 > total {
            winners.first().copied()
        } else {
            match b.config.depth_tie_policy {
                DepthTiePolicy::Pending => {
                    reason = Some("no strict majority depth; tie/plurality policy pending");
                    None
                }
                DepthTiePolicy::Shallower => winners.first().copied(),
                DepthTiePolicy::Deeper => winners.last().copied(),
            }
        };
        if depth == Some(0) {
            reason=Some("wide-area table has no explicit Periscope row; ordinary short-pass MAD remains separate");
        }
        if let Some(reason) = reason {
            trace(
                s,
                b.observer,
                "5.7.2.wide_mad_pending",
                json!({"identity":identity,"reason":reason,"inside_seconds":total,"tactical_seconds":x.turn_seconds,"depth_seconds":x.depth_seconds}),
            );
            continue;
        }
        let Some(depth) = depth else { continue };
        let probability = match super::wide_mad_probability(area(&b.config), depth, turns) {
            Ok(p) => p,
            Err(reason) => {
                trace(
                    s,
                    b.observer,
                    "5.7.2.wide_mad_pending",
                    json!({"identity":identity,"reason":reason}),
                );
                continue;
            }
        };
        let (die, reused) = super::trial(s, w.seed, w.tick, &identity, it_seconds());
        let detected = f64::from(die) <= probability;
        let endpoint = w.entity(target).filter(|t| {
            t.alive
                && t.kind == PlatformKind::Sub
                && t.side != b.side
                && t.depth_m > 0.
                && inside(&b.config, t.pos)
        });
        let can_measure = monitoring(w, s, &b) && endpoint.is_some();
        let pending = detected && !can_measure;
        trace(
            s,
            b.observer,
            "5.7.2.wide_mad_result",
            json!({"identity":identity,"field":x.field,"truth_target":target,"area_nm2":area(&b.config),"tactical_turns_inside":turns,"inside_seconds":total,"depth":depth,"depth_seconds":x.depth_seconds,"probability":probability,"d100":die,"reused":reused,"detected":detected,"report_pending":pending,"pending_reason":if pending{Some("successful IT search has no qualifying in-field endpoint measurement; no truth refresh")}else{None}}),
        );
        if !detected || !can_measure {
            continue;
        }
        let target = endpoint.unwrap();
        let observer = w.entity(b.observer).unwrap();
        let report = SensorReport {
            bearing_alternatives: Vec::new(),
            observer: b.observer,
            target: target.id,
            sensor: b.config.sensor.clone(),
            at: w.tick,
            observer_position: observer.pos,
            bearing_deg: observer.pos.bearing_to(target.pos),
            measured_position: Some(target.pos),
            bearing_error_deg: None,
            sensor_kind: SensorKind::Magnetic,
            receiver: receiver(s, &b.class, &b.config.sensor).cloned(),
        };
        let mut scan = ScanDecision::new(true, false, json!({"rule":"5.7.2","identity":identity}));
        scan.known_category = Some(PlatformKind::Sub);
        scan.fix_quality = Some("good".into());
        reports.push((scan, report));
    }
    reports
}
/// Only actual locally successful native reports enter this clock. Remote/held
/// contacts and command aimpoints never manufacture a magnetic success.
pub fn record_success(w: &World, s: &mut super::State, report: &SensorReport) {
    if report.at != w.tick
        || report.sensor_kind != SensorKind::Magnetic
        || report.measured_position.is_none()
        || !matches!(&report.receiver,Some(Receiver::Mad{source,..}) if !source.trim().is_empty())
    {
        return;
    }
    let Some(o) = w.entity(report.observer).filter(|e| {
        e.alive
            && e.kind == PlatformKind::Aircraft
            && e.sensors.iter().any(|i| i.sensor == report.sensor)
    }) else {
        return;
    };
    if !w
        .db
        .sensors
        .get(&report.sensor)
        .is_some_and(|d| d.kind == SensorKind::Magnetic)
        || crate::sensors::sensor_damaged(o, &report.sensor)
        || !crate::rules::expanded_damage2026::sensor_operational(w, o.id, &report.sensor)
            .unwrap_or(false)
    {
        return;
    }
    let key = format!(
        "{}:{}:{}",
        report.observer.0, report.sensor, report.target.0
    );
    let fix=MadFix{report:report.clone(),usable_turn:report.at.div_euclid(tac_seconds())+1,source:"English2026 5.7.1/5.7.2 PDF60-61: actual native MAD success; Good and attack in next Tactical Turn".into()};
    if let Some(h) = s.wide_mad.fixes.get_mut(&key) {
        if fix.usable_turn > h.latest.usable_turn {
            h.previous = Some(h.latest.clone());
        }
        if fix.report.at >= h.latest.report.at {
            h.latest = fix;
        }
    } else {
        s.wide_mad.fixes.insert(
            key,
            FixHistory {
                latest: fix,
                previous: None,
                observer_class: o.class.clone(),
                side: o.side,
            },
        );
    }
}
/// Retain the already measured side fix until its printed attack turn ends.
/// No current target geometry, new report timestamp or continuous tracking is
/// produced. Generic icon decay cannot erase an earned next-turn MAD datum.
pub fn retained_contact_targets(w:&World,side:Side)->BTreeSet<EntityId> {
    if w.rules.profile!=crate::rules::damage_runtime::RuleProfile::Rts2026Full {
        return BTreeSet::new();
    }
    let Some(s)=w.rules.detection.as_ref() else{return BTreeSet::new()};
    let current=w.tick.div_euclid(tac_seconds());
    s.wide_mad.fixes.values().filter(|h|h.side==side)
        .flat_map(|h|std::iter::once(&h.latest).chain(h.previous.iter()))
        .filter(|f|f.report.at<=w.tick&&current<=f.usable_turn)
        .map(|f|f.report.target).collect()
}
pub fn good_fix(
    w: &World,
    actor: EntityId,
    target: EntityId,
    sensor: Option<&str>,
) -> Option<MadFix> {
    let s = w.rules.detection.as_ref()?;
    let o = w.entity(actor).filter(|e| e.alive)?;
    if !w.sides[o.side.index()]
        .contacts
        .values()
        .any(|c| c.about == target && !c.stale)
    {
        return None;
    }
    let current = w.tick.div_euclid(tac_seconds());
    s.wide_mad
        .fixes
        .values()
        .filter(|h| h.observer_class == o.class && h.side == o.side)
        .flat_map(|h| std::iter::once(&h.latest).chain(h.previous.iter()))
        .filter(|f| {
            f.report.observer == actor
                && f.report.target == target
                && f.usable_turn == current
                && sensor.is_none_or(|i| i == f.report.sensor)
                && f.report.at <= w.tick
                && o.sensors.iter().any(|i| i.sensor == f.report.sensor)
        })
        .max_by_key(|f| f.report.at)
        .cloned()
}
/// Guard the old side-located fallback when this observer has only a MAD fix.
/// An independent native measured receiver remains a valid different source.
pub fn only_mad_waiting(w: &World, actor: EntityId, target: EntityId) -> bool {
    if good_fix(w, actor, target, None).is_some() {
        return false;
    }
    let Some(s) = w.rules.detection.as_ref() else {
        return false;
    };
    let has_mad = s
        .wide_mad
        .fixes
        .values()
        .any(|h| h.latest.report.observer == actor && h.latest.report.target == target);
    if !has_mad {
        return false;
    }
    let fresh = crate::rules::firecontrol2026::tables()["execution_revision"]["own_motion"]
        ["report_freshness_s"]
        .as_i64()
        .unwrap();
    let cid = w.entity(actor).and_then(|o| w.find_contact(o.side, target));
    let other = w.rules.aaw.as_ref().is_some_and(|a| {
        a.firecontrol.tracks.values().any(|t| {
            t.observer == actor
                && t.target == target
                && t.report.as_ref().is_some_and(|r| {
                    r.observer == actor
                        && r.target == target
                        && r.sensor_kind != SensorKind::Magnetic
                        && r.measured_position.is_some()
                        && r.at <= w.tick
                        && w.tick - r.at <= fresh
                        && cid.is_some_and(|cid| {
                            crate::rules::communications2026::native_report_view(
                                w, actor, cid, &t.sensor,
                            )
                            .is_ok()
                        })
                })
        })
    });
    if other {
        return false;
    }
    if crate::rules::communications2026::received_located_report(w, actor, target)
        .ok()
        .flatten()
        .is_some_and(|r| {
            r.measured_position.is_some()
                && (r.sensor_kind != SensorKind::Magnetic
                    || r.at.div_euclid(tac_seconds()) < w.tick.div_euclid(tac_seconds()))
        })
    {
        return false;
    }
    if crate::rules::communications2026::received_solution(w, actor, target)
        .ok()
        .flatten()
        .is_some_and(|r| {
            r.domain == crate::rules::communications2026::FcsDomain::Asw
                && r.aim.is_some()
                && r.quality != crate::rules::firecontrol2026::Quality::NA
        })
    {
        return false;
    }
    true
}
pub fn commit_traces(w: &mut World) {
    let traces = w
        .rules
        .detection
        .as_mut()
        .map(|s| std::mem::take(&mut s.wide_mad.traces))
        .unwrap_or_default();
    for t in traces {
        w.emit(
            EventKind::RulesEvent {
                domain: "detection2026".into(),
                rule: t.rule,
                side: None,
                unit: Some(t.actor),
                contact: None,
                detail: t.detail.to_string(),
            },
            Visibility::Referee,
        );
    }
}
pub fn own_view(w: &World, actor: EntityId) -> Value {
    let fields=w.rules.detection.as_ref().map(|s|s.wide_mad.fields.iter().filter(|(_,b)|b.observer==actor)
        .map(|(id,b)|json!({"field":id,"source":b.config.source,"enabled":b.config.enabled,"sensor":b.config.sensor,"area_nm2":area(&b.config)})).collect::<Vec<_>>()).unwrap_or_default();
    json!({"fields":fields})
}

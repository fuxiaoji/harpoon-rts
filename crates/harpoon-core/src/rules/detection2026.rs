//! Executable Harpoon V 2026 chapter 5 mechanisms. Enabled only by opt-in v4.
//! All fitted sensor/target metadata is explicit. No legacy estimated range,
//! noise_db, generic class name, or physical size is a substitute for Annex data.
//! Dice live in referee state; callers must never publish failed-scan traces.
use crate::geom::angle_diff;
use crate::types::PlatformKind;
use crate::{EntityId, Rng, Vec2, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
#[cfg(test)]
#[path = "detection_emergency_source_contract.rs"]
mod emergency_source_contract_20261024;
#[path="detection2026/array_optional.rs"]
pub mod array_optional;
#[path="detection2026/array_optional_world.rs"]
pub mod array_optional_world;
#[path="detection2026/wide_mad.rs"]
pub mod wide_mad;
#[path="detection2026/soks.rs"]
pub mod soks;
#[path="detection2026/laser.rs"]
pub mod laser;
#[path="detection2026/sonobuoys.rs"]
pub mod sonobuoys;
#[path="detection2026/visual_id.rs"]
pub mod visual_id;
pub mod squalls;
#[path="detection2026/visual_signaling.rs"]
pub mod visual_signaling;

pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/detection-2026.json"))
            .expect("verified detection tables")
    })
}
fn n(v: &Value) -> f64 {
    v.as_f64().expect("verified numeric source cell")
}
fn i(v: &Value) -> i32 {
    v.as_i64().expect("verified integer source cell") as i32
}
fn valid_nonnegative(v: f64) -> Result<(), String> {
    if v.is_finite() && v >= 0.0 {
        Ok(())
    } else {
        Err("nonfinite or negative input".into())
    }
}
pub fn fingerprint() -> String {
    let b = serde_json::to_vec(tables()).unwrap();
    format!(
        "fnv64:{:016x}",
        b.iter().fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64)
            .wrapping_mul(0x100000001b3))
    )
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub squalls:Option<squalls::State>,
    pub config: Configuration,
    pub wide_mad:wide_mad::State,
    pub soks:soks::State,
    pub laser:laser::State,
    pub sonobuoys:sonobuoys::State,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub visual_identification:Option<visual_id::State>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub visual_signaling:Option<visual_signaling::State>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub optional_arrays:Option<array_optional_world::State>,
    pub ultra_quiet: BTreeSet<EntityId>,
    pub transients: BTreeMap<EntityId, (i64, i32)>,
    /// Stable weapon identity recorded at the production launch event. The
    /// public release event keeps its historical display-name contract.
    pub release_weapons: BTreeMap<u64, ReleaseWeapon>,
    pub unresolved_transients: BTreeMap<EntityId, (i64, String)>,
    pub last_event_seq: u64,
    pub fired: BTreeSet<EntityId>,
    pub reported: BTreeMap<String, (i64, String)>,
    pub categories: BTreeMap<String, PlatformKind>,
    pub group_representatives: BTreeMap<String, EntityId>,
    pub modes: BTreeMap<String, (i64, bool)>,
    pub arrays: BTreeMap<String, ArrayJob>,
    pub array_positions: BTreeMap<String, Vec2>,
    pub tracks: BTreeMap<String, Track>,
    #[serde(default,skip_serializing_if="BTreeMap::is_empty")]
    pub sonar_alert_history:BTreeMap<String,SonarAlertHistory>,
    pub trials: BTreeMap<String, Trial>,
    pub masts: BTreeMap<EntityId, Mast>,
    pub groups: BTreeMap<String, Vec<EntityId>>,
    pub emissions: BTreeMap<EntityId, Emission>,
    pub supersonic: BTreeMap<EntityId, bool>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct SonarAlertHistory {pub epoch:i64,pub latest:i64,pub previous:Option<i64>}
/// Same-platform arrays share a successful sonar detection only in subsequent
/// Tactical Turns. A separate previous timestamp prevents per-second scans
/// from erasing a modifier already earned for the current search epoch.
fn ordinary_sonar_alerted(w:&World,state:&State,observer:EntityId,target:EntityId)->bool {
    if let Some(alerted)=sonobuoys::sonar_alerted(w,state,observer,target){return alerted;}
    let Some(h)=state.sonar_alert_history.get(&format!("{}:{}",observer.0,target.0)) else{return false};
    let at=if h.epoch<w.tick.div_euclid(180){Some(h.latest)}else{h.previous};
    at.is_some_and(|at|at<=w.tick&&w.tick-at<=720)
}
pub fn record_sonar_successes(w:&mut World,successes:&[(EntityId,EntityId,i64)]) {
    let ordinary:Vec<_>=successes.iter().filter(|(observer,_,_)|!sonobuoys::is_sensor_float(w,*observer)).copied().collect();
    let Some(s)=w.rules.detection.as_mut() else{return};
    for (observer,target,at) in &ordinary {
        let epoch=at.div_euclid(180);let key=format!("{}:{}",observer.0,target.0);
        if let Some(h)=s.sonar_alert_history.get_mut(&key) {if epoch>h.epoch{h.previous=Some(h.latest);h.epoch=epoch;}h.latest=h.latest.max(*at);}
        else{s.sonar_alert_history.insert(key,SonarAlertHistory{epoch,latest:*at,previous:None});}
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReleaseWeapon {
    pub projectile: EntityId,
    pub weapon: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Configuration {
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub squalls:Option<squalls::Configuration>,
    /// Explicit source metadata for one installed radar emitter per named
    /// unit. This never grants a radar or authorizes a fabricated emission.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub emissions: BTreeMap<String, EmitterConfiguration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub wide_mad:Option<wide_mad::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub soks:Option<soks::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub laser_sensors:Option<laser::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub sonobuoys:Option<sonobuoys::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub visual_identification:Option<visual_id::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub visual_signaling:Option<visual_signaling::Configuration>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub optional_towed_array:Option<array_optional::Configuration>,
    /// key is observer platform class + ':' + fitted SensorInstance.sensor.
    pub bindings: BTreeMap<String, Receiver>,
    pub targets: BTreeMap<String, TargetProfile>,
    /// Explicit scenario environment. Absence blocks nonradar scanning.
    pub environment: Option<Environment>,
    pub propagation: BTreeMap<String, Propagation>,
    pub duct: Option<Duct>,
    pub propulsion: BTreeMap<String, String>,
    pub quiet_launches: BTreeSet<String>,
    pub max_masts: BTreeMap<String, u8>,
    pub target_heights_m: BTreeMap<String, f64>,
    pub camouflage: BTreeMap<String, usize>,
    /// Explicit fitted airframe type; a generic Aircraft class does not prove
    /// that it can hover or operate a dipping sonar from the water.
    pub dipping_platforms: BTreeMap<String, DippingPlatform>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DippingPlatform {
    Helicopter,
    FlyingBoat,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TargetProfile {
    pub source: String,
    /// physical A=0, B=1, C=2, D=3, E=4, F=5, G=6, H=7
    pub size: usize,
    /// noise Loud=0 Noisy=1 Quiet=2 VeryQuiet=3 ExtremelyQuiet=4
    pub noise: Option<usize>,
    pub anechoic: bool,
    pub titanium: bool,
    pub pumpjet: bool,
    pub stealthy_ir: bool,
    pub radar_signature: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Receiver {
    RadarFeatures {
        source: String,
        sar: bool,
        isar: bool,
        nctr: bool,
    },
    Tv {
        source: String,
        low_light: bool,
        range_nm: f64,
        arc_deg: f64,
    },
    Visual {
        source: String,
    },
    Ir {
        source: String,
        generation: usize,
        mode: IrMode,
        arc_deg: f64,
    },
    Es {
        source: String,
        generation: usize,
        targeting_complex: bool,
        rwr: bool,
        bearing_error_deg: Option<f64>,
        els: bool,
    },
    Sonar {
        source: String,
        generation: usize,
        passive_range_nm: Option<f64>,
        active_range_nm: Option<f64>,
        frequency: Frequency,
        mount: SonarMount,
        active_torpedo_detection: bool,
        high_speed_tow: bool,
    },
    AcousticIntercept {
        source: String,
        generation: usize,
    },
    Mad {
        source: String,
        range_nm: f64,
        western: bool,
    },
    LaserRangefinder {
        source: String,
        range_nm: Option<f64>,
    },
}
impl Receiver {
    pub fn source(&self) -> &str {
        match self {
            Self::RadarFeatures { source, .. }
            | Self::Tv { source, .. }
            | Self::Visual { source }
            | Self::Ir { source, .. }
            | Self::Es { source, .. }
            | Self::Sonar { source, .. }
            | Self::AcousticIntercept { source, .. }
            | Self::Mad { source, .. }
            | Self::LaserRangefinder { source, .. } => source,
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Vlf,
    Lf,
    Lmf,
    Mf,
    Hf,
}
impl Frequency {
    fn idx(self) -> usize {
        match self {
            Self::Vlf => 0,
            Self::Lf => 1,
            Self::Lmf => 2,
            Self::Mf => 3,
            Self::Hf => 4,
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IrMode {
    Scanning,
    Flir,
    Irst,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SonarMount {
    Bow,
    Keel,
    Flank,
    ForwardSailFixed,
    AftSailFixed,
    ForwardDigital,
    AftDigital,
    Towed,
    Vds,
    Dipping,
    Buoy,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Mast {
    pub count: u8,
    pub raised_since: i64,
}
/// Generic Masts is a declared aggregate count. A battery snorkel is one
/// physical mast within that aggregate, so max avoids inventing a union of
/// unidentified pieces of hardware. A historical unknown clock stays pending.
fn effective_mast(w:&World,state:&State,target:EntityId)->Result<Option<Mast>,String>{
    let physical=w.entity(target).is_some_and(|e|e.kind!=crate::types::PlatformKind::Sub || e.depth_m<=25. && e.speed_kts<=15.);
    let declared=state.masts.get(&target).cloned();
    if !physical {return Ok(declared.map(|mut m|{m.count=0;m}));}
    if let Some((true,since))=super::battery2026::raised_snorkel(w,target){
        if let Some(m)=declared.filter(|m|m.count>0){return Ok(Some(m));}
        return since.map(|raised_since|Some(Mast{count:1,raised_since})).ok_or_else(||"3.2/5.2 battery snorkel is raised but its historical continuous exposure time is unknown".into());
    }
    Ok(declared)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Emission {
    pub source: String,
    pub emitting: bool,
    pub weapon_radar: bool,
    pub lpi: bool,
    pub radar_role: String,
    pub max_range_nm: f64,
    pub antenna_height_m: f64,
}
/// Static provenance and characteristics. Emission on/off is always read from
/// the named unit's actual installed sensor and runtime radar/power state.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmitterConfiguration {
    pub source: String,
    pub sensor: String,
    pub weapon_radar: bool,
    pub lpi: bool,
    pub radar_role: String,
    pub max_range_nm: f64,
    pub antenna_height_m: f64,
}
fn emitter_hardware_operating(w: &World, id: EntityId, sensor: &str) -> bool {
    w.entity(id).is_some_and(|e| {
        e.alive
            && e.systems.radar > 0.05
            && crate::sensors::radar_emitting(e)
            && !crate::sensors::sensor_damaged(e, sensor)
            // `active` is the active-sonar pinging bit. Radar's real on/off
            // state is radar_emitting above, not that sonar-only field.
            && e.sensors.iter().any(|s| s.sensor == sensor && s.is_active_kind(crate::db::SensorKind::Radar))
            && w.db.sensors.get(sensor).is_some_and(|s| s.kind == crate::db::SensorKind::Radar)
            && super::expanded_damage2026::sensor_operational(w, id, sensor).unwrap_or(false)
    })
}
/// Load-time adapter. Validate every authored row before replacing any runtime
/// metadata; invalid configuration leaves the entire World unchanged.
pub fn initialize_emissions(w: &mut World) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    if let Some(config) = &w.context.hf_es_2026 {
        config.validate()?;
        for (name, fit) in &config.receivers {
            let units: Vec<_> = w.entities.values().filter(|e| e.name == *name).collect();
            if units.len()!=1 || !units[0].sensors.iter().any(|s|s.sensor==fit.sensor)
                || !w.db.sensors.get(&fit.sensor).is_some_and(|s|s.kind==crate::db::SensorKind::Esm) {
                return Err("6.3.9 authored HF-ES fit requires one exact body and actual fitted ES sensor".into());
            }
        }
    }
    let config = w.rules.detection.as_ref().ok_or("5.3 detection state absent")?.config.emissions.clone();
    let mut prepared = BTreeMap::new();
    for (name, row) in config {
        if name.trim().is_empty() || row.source.trim().is_empty() || row.sensor.trim().is_empty()
            || !row.max_range_nm.is_finite() || row.max_range_nm <= 0.
            || !row.antenna_height_m.is_finite() || row.antenna_height_m < 0.
            || !matches!(row.radar_role.as_str(), "AS" | "3D" | "LAS" | "SS" | "SS-T" | "Nav"
                | "AI" | "LD/SD" | "HF" | "RO" | "MFC" | "GFC" | "TF" | "TA" | "SLAR")
        {
            return Err(format!("5.3 invalid explicit emitter source/parameters for {name}"));
        }
        let units: Vec<_> = w.entities.values().filter(|e| e.name == name).collect();
        if units.len() != 1 {
            return Err(format!("5.3 emitter unit {name} must resolve uniquely"));
        }
        let unit = units[0];
        if !unit.sensors.iter().any(|s| s.sensor == row.sensor)
            || !w.db.sensors.get(&row.sensor).is_some_and(|s| s.kind == crate::db::SensorKind::Radar)
        {
            return Err(format!("5.3 emitter {name}:{} requires an actually installed radar", row.sensor));
        }
        prepared.insert(unit.id, Emission {
            source: row.source,
            emitting: emitter_hardware_operating(w, unit.id, &row.sensor),
            weapon_radar: row.weapon_radar,
            lpi: row.lpi,
            radar_role: row.radar_role,
            max_range_nm: row.max_range_nm,
            antenna_height_m: row.antenna_height_m,
        });
    }
    let visual_identification=visual_id::initialize(w.rules.detection.as_ref().unwrap().config.visual_identification.as_ref())?;
    w.rules.detection.as_mut().unwrap().visual_identification=visual_identification;
    let visual_signaling=visual_signaling::initialize(w,w.rules.detection.as_ref().unwrap().config.visual_signaling.as_ref())?;
    w.rules.detection.as_mut().unwrap().visual_signaling=visual_signaling;
    squalls::initialize(w)?;
    laser::initialize(w)?;
    soks::initialize(w)?;
    wide_mad::initialize(w)?;
    sonobuoys::initialize(w)?;
    w.rules.detection.as_mut().unwrap().emissions = prepared;
    Ok(())
}
/// A physically stowed observer cannot bank deployment, identification or
/// continuous targeting work. Other observers' contacts and report histories
/// remain their knowledge; no death or fictitious detection is generated.
pub fn stow_platform(w:&mut World,id:EntityId)->Result<(),String>{
    if !active(w){return Ok(());}
    if !w.entity(id).is_some_and(|e|e.alive){return Err("5 physical sensor stowage requires the actual live body before removal".into());}
    let Some(s)=w.rules.detection.as_mut() else{return Ok(())};
    let observer_name=id.0.to_string();
    let prefix=format!("{}:",id.0);
    let observer_trial=|key:&str|{
        if key.starts_with(&prefix){return true;}
        let mut parts=key.split(':');let rule=parts.next().unwrap_or_default();
        rule.starts_with("5.")&&rule!="5.8.cloud"&&parts.next()==Some(observer_name.as_str())
    };
    s.masts.remove(&id);s.emissions.remove(&id);s.transients.remove(&id);s.unresolved_transients.remove(&id);s.supersonic.remove(&id);s.fired.remove(&id);
    s.arrays.retain(|key,job|{
        if !key.starts_with(&prefix){return true;}
        // A severed/lost cable is persistent physical damage, never restored
        // merely by a recover/relaunch cycle.
        if job.lost{job.deployed=false;job.recovering=false;return true;}false
    });
    s.array_positions.retain(|key,_|!key.starts_with(&prefix));
    s.modes.retain(|key,_|!key.starts_with(&prefix));s.tracks.retain(|key,_|!key.starts_with(&prefix));
    s.sonar_alert_history.retain(|key,_|!key.starts_with(&prefix));
    s.trials.retain(|key,_|!observer_trial(key));s.reported.retain(|key,_|!observer_trial(key));
    array_optional_world::stow_platform(s,id);
    s.wide_mad.fixes.retain(|_,h|h.latest.report.observer!=id);
    let fields:BTreeSet<_>=s.wide_mad.fields.iter().filter(|(_,b)|b.observer==id).map(|(k,_)|k.clone()).collect();
    s.wide_mad.exposures.retain(|_,x|!fields.contains(&x.field));
    if let Some(aaw)=w.rules.aaw.as_mut(){aaw.firecontrol.tracks.retain(|_,t|t.observer!=id);}
    Ok(())
}
/// Rebind only this returning body's configured real emitter. Re-running the
/// global initializer would reject other correctly absent inventory bodies.
pub fn resume_platform(w:&mut World,id:EntityId)->Result<(),String>{
    if !active(w){return Ok(());}
    let entity=w.entity(id).filter(|e|e.alive).ok_or("5 sensor resume requires the same live physical body after insertion")?;
    let row=w.rules.detection.as_ref().and_then(|s|s.config.emissions.get(&entity.name)).cloned();
    let prepared=if let Some(row)=row {
        if row.source.trim().is_empty()||row.sensor.trim().is_empty()||!row.max_range_nm.is_finite()||row.max_range_nm<=0.
            ||!row.antenna_height_m.is_finite()||row.antenna_height_m<0.
            ||!matches!(row.radar_role.as_str(),"AS"|"3D"|"LAS"|"SS"|"SS-T"|"Nav"|"AI"|"LD/SD"|"HF"|"RO"|"MFC"|"GFC"|"TF"|"TA"|"SLAR")
            ||w.entities.values().filter(|e|e.name==entity.name).count()!=1
            ||!entity.sensors.iter().any(|s|s.sensor==row.sensor)
            ||!w.db.sensors.get(&row.sensor).is_some_and(|s|s.kind==crate::db::SensorKind::Radar){return Err("5.3 returning body lacks its unique actual sourced emitter hardware".into());}
        Some(Emission{source:row.source,emitting:emitter_hardware_operating(w,id,&row.sensor),weapon_radar:row.weapon_radar,lpi:row.lpi,radar_role:row.radar_role,max_range_nm:row.max_range_nm,antenna_height_m:row.antenna_height_m})
    }else{None};
    stow_platform(w,id)?;
    if let (Some(s),Some(emission))=(w.rules.detection.as_mut(),prepared){s.emissions.insert(id,emission);}
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Environment {
    pub source: String,
    pub sea_state: usize,
    /// None=0, drizzle/fog=1, light=2, moderate=3, heavy=4, torrential=5
    pub precipitation: usize,
    pub visibility_percent: u8,
    /// None=0 Light=1 Moderate=2 Heavy=3
    pub shipping: usize,
    pub biologics: usize,
    pub water_depth_m: f64,
    pub layer: Option<usize>,
    /// Open=0 MarginalIceZone=1 PackIce=2
    pub ice: usize,
    pub cloud: Option<Cloud>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cloud {
    pub base_m: f64,
    pub top_m: f64,
    pub coverage_percent: u8,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Track {
    pub last_success: Option<i64>,
    pub held: bool,
    pub els_fixes: u32,
    pub last_els_position: Option<Vec2>,
    pub last_classification_window: Option<i64>,
    pub last_els_window: Option<i64>,
    pub first_success: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trial {
    pub window: i64,
    pub d100: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanDecision {
    /// Referee point of a committed transient emission, not a measured fix.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub source_point: Option<Vec2>,
    /// An actual source-qualified sonar search, including failed and zero-Ph trials.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub sonar_attempt:Option<super::communications2026::SonarAttempt>,
    pub detected: bool,
    pub known_category: Option<PlatformKind>,
    pub group: Option<String>,
    pub bearing_only: bool,
    pub identify_class: bool,
    /// Apparent source identity, including a simulator's imitated submarine.
    pub class_label: Option<String>,
    pub range_known: bool,
    pub bearing_error_deg: Option<f64>,
    /// Fair/Good here describe ELS/MAD fix evidence, not an omniscient truth coordinate.
    pub fix_quality: Option<String>,
    pub trace: Value,
}
impl ScanDecision {
    fn new(detected: bool, bearing_only: bool, trace: Value) -> Self {
        Self {
            source_point:None,
            sonar_attempt:None,
            detected,
            known_category: None,
            group: None,
            bearing_only,
            identify_class: false,
            class_label: None,
            range_known: detected && !bearing_only,
            bearing_error_deg: None,
            fix_quality: None,
            trace,
        }
    }
}
/// The geometry one particular successful receiver actually reported. A
/// bearing-ray display anchor is deliberately not a measured target position.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorReport {
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub bearing_alternatives:Vec<f64>,
    pub observer: EntityId,
    pub target: EntityId,
    pub sensor: String,
    pub at: i64,
    pub observer_position: Vec2,
    pub bearing_deg: f64,
    pub measured_position: Option<Vec2>,
    pub bearing_error_deg: Option<f64>,
    pub sensor_kind: crate::db::SensorKind,
    pub receiver: Option<Receiver>,
}
impl SensorReport {
    pub fn plot_position(&self) -> Vec2 {
        self.measured_position
            .unwrap_or_else(|| self.observer_position.advanced(self.bearing_deg, 1.0))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportGeometry {
    pub observer_position: Vec2,
    pub bearing_deg: f64,
    pub measured_position: Option<Vec2>,
    pub bearing_error_deg: Option<f64>,
}
/// The same source projection used for combat contacts and independent physical
/// objects. The target tag is referee-only; no fake combat entity is required.
pub fn project_point_report(
    w: &World,
    state: &mut State,
    observer: &crate::world::Entity,
    point: Vec2,
    private_target: Value,
    trial_key: &str,
    sensor: &str,
    scan: &ScanDecision,
) -> (ReportGeometry, Option<Value>) {
    let mut bearing = observer.pos.bearing_to(point);
    let mut evidence = None;
    if let Some(error) = scan.bearing_error_deg {
        let (roll, reused) = trial(state, w.seed, w.tick, trial_key, 180);
        bearing =
            crate::geom::normalize_deg(bearing + error * (2.0 * (roll as f64 - 0.5) / 100.0 - 1.0));
        if !reused {
            evidence = Some(
                json!({"truth_target":private_target,"sensor":sensor,"d100":roll,"half_width_deg":error,"reported_bearing":bearing,"adaptation":"uniform within printed angular uncertainty; no range inference"}),
            );
        }
    }
    let measured_position = scan.range_known.then(|| {
        if scan.bearing_error_deg.is_some() {
            observer.pos.advanced(bearing, observer.pos.range_to(point))
        } else {
            point
        }
    });
    (
        ReportGeometry {
            observer_position: observer.pos,
            bearing_deg: bearing,
            measured_position,
            bearing_error_deg: scan.bearing_error_deg,
        },
        evidence,
    )
}
/// Shared projection for the public fused icon and private receiver evidence.
/// Reuses the existing per-receiver angular die; never samples truth motion or
/// invents a range for a bearing-only receiver.
pub fn project_report(
    w: &World,
    state: &mut State,
    observer: &crate::world::Entity,
    target: &crate::world::Entity,
    sensor: &str,
    scan: &ScanDecision,
) -> (SensorReport, Option<Value>) {
    let (geometry, evidence) = project_point_report(
        w,
        state,
        observer,
        scan.source_point.unwrap_or(target.pos),
        json!(target.id),
        &format!("5.bearing:{}:{sensor}:{}", observer.id.0, target.id.0),
        sensor,
        scan,
    );
    (
        SensorReport {
            bearing_alternatives:Vec::new(),
            observer: observer.id,
            target: target.id,
            sensor: sensor.into(),
            at: w.tick,
            observer_position: geometry.observer_position,
            bearing_deg: geometry.bearing_deg,
            measured_position: geometry.measured_position,
            bearing_error_deg: geometry.bearing_error_deg,
            sensor_kind: w.db.sensors[sensor].kind,
            receiver: state
                .config
                .bindings
                .get(&format!("{}:{sensor}", observer.class))
                .cloned(),
        },
        evidence,
    )
}
/// Deterministic one roll per receiver/target/purpose/180-second window. Calling
/// every second, or reloading, must not turn a 25% fleeting-mast trial into certainty.
pub fn trial(state: &mut State, seed: u64, now: i64, key: &str, period_s: i64) -> (u8, bool) {
    assert!(period_s > 0);
    let window = now.div_euclid(period_s);
    if let Some(t) = state.trials.get(key).filter(|t| t.window == window) {
        return (t.d100, true);
    }
    let mut rng = Rng::new(seed).derive(Rng::hash_tag(window, 0, key));
    let d100 = 1 + (rng.next_f64() * 100.0) as u8;
    state.trials.insert(key.into(), Trial { window, d100 });
    (d100, false)
}

/// §5.2.2: indices Large=0..Stealthy=4. Formation membership is explicit,
/// never inferred from nearby aircraft, and cannot exceed Large Annex range.
pub fn group_signature(
    base: usize,
    external_ordnance: bool,
    group_size: usize,
) -> Result<usize, String> {
    if base > 4 || group_size == 0 {
        return Err("5.2.2: invalid signature or empty group".into());
    }
    let base = if external_ordnance { base.min(2) } else { base };
    let shift = if group_size >= 9 {
        2
    } else if group_size >= 3 {
        1
    } else {
        0
    };
    Ok(base.saturating_sub(shift))
}
pub fn radar_mast_probability(sea_state: usize, isar: bool, raised_s: i64) -> Result<u8, String> {
    if sea_state > 9 || raised_s < 0 {
        return Err("5.2.1: invalid mast environment/time".into());
    }
    Ok(if sea_state > 4 {
        0
    } else if raised_s >= 180 {
        100
    } else if isar {
        50
    } else {
        25
    })
}
pub fn radar_identifies(sar: bool, isar: bool, nctr: bool, kind: PlatformKind) -> bool {
    match kind {
        PlatformKind::Base => sar,
        PlatformKind::Ship => isar,
        PlatformKind::Aircraft => nctr,
        _ => false,
    }
}
/// Printed duct tables; observer/target Large=0..Periscope=4.
pub fn duct_horizon(surface: bool, observer: usize, target: usize) -> Result<f64, String> {
    if observer > 4 || target > 4 {
        return Err("5.2.9.5: duct size class missing".into());
    }
    Ok(n(&tables()["radar"]
        [if surface { "duct_surface" } else { "duct_evap" }]
        [observer][target]))
}
pub fn evaporative_max_size(roll: u8, season: usize, wind_kts: f64) -> Result<usize, String> {
    if !(1..=100).contains(&roll) || season > 2 {
        return Err("5.2.9.5: bad season/d100".into());
    }
    valid_nonnegative(wind_kts)?;
    if wind_kts > 30.0 {
        return Err("5.2.9.5: wind prevents evaporative duct".into());
    }
    let modified = roll as i32 - [0, 15, 30][season] - if wind_kts <= 10.0 { 20 } else { 0 };
    let limits = &tables()["radar"]["evap_roll_max"];
    let col = (0..5).find(|j| modified <= i(&limits[*j])).unwrap();
    Ok(i(&tables()["radar"]["evap_max_size"][col]) as usize)
}
pub fn surface_duct_chance(
    season: usize,
    near_land: bool,
    climate: i32,
    wind_kts: f64,
    cloud_percent: u8,
) -> Result<i32, String> {
    if season > 2 || !(-1..=1).contains(&climate) {
        return Err("5.2.9.5: invalid season/climate".into());
    }
    valid_nonnegative(wind_kts)?;
    if wind_kts > 5.0 || cloud_percent > 25 {
        return Ok(0);
    }
    Ok(i(&tables()["radar"]["surface_season_chance"][season])
        + if near_land { 10 } else { 0 }
        + climate * 10)
}
/// §5.3 multiplier includes explicit SS-T bonuses; LPI subtracts 1.0, not half range.
pub fn es_range(
    horizon_nm: f64,
    generation: usize,
    targeting: bool,
    role: &str,
    radar_range_nm: f64,
    lpi: bool,
) -> Result<f64, String> {
    valid_nonnegative(horizon_nm)?;
    valid_nonnegative(radar_range_nm)?;
    if !(1..=6).contains(&generation) {
        return Err("5.3: ES generation missing".into());
    }
    let bonus = if !targeting {
        0.0
    } else {
        match role {
            "AS" | "3D" if radar_range_nm >= 150.0 => 0.7,
            "AS" | "3D" => 0.4,
            "LAS" | "SS" | "SS-T" | "Nav" => 0.3,
            _ => 0.0,
        }
    };
    Ok(horizon_nm
        * (n(&tables()["es"]["generation_multiplier"][generation - 1]) + bonus
            - if lpi { 1.0 } else { 0.0 }))
}
fn speed_col(speed: f64) -> Result<usize, String> {
    valid_nonnegative(speed)?;
    Ok(if speed <= 8.0 {
        0
    } else if speed <= 15.0 {
        1
    } else if speed <= 20.0 {
        2
    } else if speed <= 25.0 {
        3
    } else if speed <= 30.0 {
        4
    } else {
        5
    })
}
pub fn sonar_speed_modifier(
    active: bool,
    target_kts: f64,
    observer_kts: f64,
    high_speed_tow: bool,
) -> Result<Option<i32>, String> {
    let t = speed_col(target_kts)?;
    let o = speed_col((observer_kts - if high_speed_tow { 7.0 } else { 0.0 }).max(0.0))?;
    let cell = &tables()["sonar"][if active {
        "active_speed"
    } else {
        "passive_speed"
    }][t][o];
    Ok(cell.as_i64().map(|v| v as i32))
}
pub fn sonar_range_modifier(total: i32) -> Result<f64, String> {
    if total < -10 {
        return Err("5.4.6: modifier total below printed -10 row; pending adjudication".into());
    }
    if total > 10 {
        return Ok((total - 1) as f64);
    }
    Ok(n(
        &tables()["sonar"]["range_modifiers"][(total + 10) as usize]
    ))
}
/// Exact printed rows, nearest modified 50% range, then next range cell >=actual.
/// Tie in nearest row is unresolved in the text and therefore blocks.
fn sonar_row(modified_50_nm: f64) -> Result<usize, String> {
    if !modified_50_nm.is_finite() || modified_50_nm <= 0. {
        return Err("5.4.6: positive finite modified50 range required".into());
    }
    let rows = tables()["sonar"]["range_probability_rows"]
        .as_array()
        .unwrap();
    // Outside the printed centre interval the unique nearest row is an
    // endpoint. Choose it before subtracting: at very large finite inputs,
    // every difference can round to the same f64 and manufacture a false tie.
    if modified_50_nm <= n(&rows[0][8]) {
        return Ok(0);
    }
    if modified_50_nm >= n(&rows[rows.len() - 1][8]) {
        return Ok(rows.len() - 1);
    }
    let mut best = 0;
    let mut diff = f64::INFINITY;
    let mut tie = false;
    for (j, row) in rows.iter().enumerate() {
        let d = (n(&row[8]) - modified_50_nm).abs();
        if d < diff - 1e-9 {
            best = j;
            diff = d;
            tie = false
        } else if (d - diff).abs() < 1e-9 {
            tie = true
        }
    }
    if tie {
        return Err("5.4.6: equally-near 50% range rows; rounding not stated".into());
    }
    Ok(best)
}
/// PDF52 explicitly rounds modified50 to the nearest printed centre cell,
/// including the unique nearest endpoint. No new row/probability is extrapolated.
pub fn sonar_selected_50(modified_50_nm: f64) -> Result<f64, String> {
    Ok(n(
        &tables()["sonar"]["range_probability_rows"][sonar_row(modified_50_nm)?][8]
    ))
}
/// Returns None outside the printed sonar range row; Some(0) is retained as
/// an in-range zero cell, allowing post-range modifiers to be applied safely.
pub fn sonar_range_cell(modified_50_nm: f64, actual_nm: f64) -> Result<Option<f64>, String> {
    valid_nonnegative(actual_nm)?;
    let best = sonar_row(modified_50_nm)?;
    let rows = tables()["sonar"]["range_probability_rows"].as_array().unwrap();
    let Some(col) = (0..17).find(|j| actual_nm <= n(&rows[best][*j]) + 1e-9) else { return Ok(None); };
    Ok(Some(n(&tables()["sonar"]["probability"][col])))
}
pub fn sonar_probability(modified_50_nm: f64, actual_nm: f64, alerted: bool) -> Result<f64, String> {
    Ok(sonar_range_cell(modified_50_nm, actual_nm)?.map_or(0.0, |base| (base + if alerted { 15.0 } else { 0.0 }).min(90.0)))
}
/// Depth bands Surface/PD=0 Shallow=1 Intermediate I=2 ... V=6 Deep+=7.
pub fn depth_band(depth: f64) -> Result<usize, String> {
    valid_nonnegative(depth)?;
    Ok(if depth <= 25.0 {
        0
    } else if depth <= 50.0 {
        1
    } else if depth <= 100.0 {
        2
    } else if depth <= 200.0 {
        3
    } else if depth <= 300.0 {
        4
    } else if depth <= 400.0 {
        5
    } else if depth <= 500.0 {
        6
    } else {
        7
    })
}
pub fn layer_modifier(
    frequency: Frequency,
    passive: bool,
    sonar_depth: f64,
    target_depth: f64,
    layer: Option<usize>,
) -> Result<i32, String> {
    let Some(layer) = layer else { return Ok(0) };
    if layer > 2 {
        return Err("5.4.7.1: invalid layer".into());
    }
    let boundary = n(&tables()["sonar"]["layer_boundaries_m"][layer]);
    let affected = match layer {
        0 => frequency.idx() >= 3,
        1 => frequency.idx() >= 2,
        _ => true,
    };
    if !affected {
        return Ok(0);
    }
    let below = sonar_depth > boundary;
    let target_below = target_depth > boundary;
    if passive && frequency == Frequency::Vlf {
        return Ok(if layer == 2 && below != target_below {
            -2
        } else {
            0
        });
    }
    let band = depth_band(target_depth)?;
    let row = if below { band.min(5) } else { band.min(3) };
    Ok(i(&tables()["sonar"]
        [if below { "layer_below" } else { "layer_above" }][row]
        [layer]))
}
/// All five frequencies can hear explosions; active transmissions require the
/// same or the immediately lower band, never arbitrary broadband bonuses.
pub fn hears_active(passive: Frequency, active: Frequency) -> bool {
    passive.idx() == active.idx() || passive.idx() + 1 == active.idx()
}
pub fn cavitating(noise: usize, speed: f64, depth: f64, pumpjet: bool) -> Result<bool, String> {
    if noise > 4 {
        return Err("5.4.6.5: unknown noise rating".into());
    }
    if pumpjet {
        return Ok(false);
    }
    let col = speed_col(speed)?.min(4);
    let c = &tables()["sonar"]["cavitation"][noise][col];
    Ok(c.as_u64()
        .is_some_and(|b| depth_band(depth).is_ok_and(|d| d <= (b as usize))))
}
fn world_cavitating(
    w: &World,
    target: &crate::world::Entity,
    noise: usize,
    pumpjet: bool,
) -> Result<bool, String> {
    // 3.1.1 applies the 5.4.6.5 modifier to acceleration/deceleration above
    // silent limits. It is one modifier even when speed/depth also cavitates.
    let maneuver = active(w)
        && target.kind == PlatformKind::Sub
        && super::navigation2026::configured(w, target.id)
        && target.ext.get("navigation.cavitation") == Some(&1.0);
    let crash_back=active(w) && super::navigation2026::configured(w,target.id)
        && target.ext.get("navigation.crash_back_cavitation")==Some(&1.0);
    Ok(crash_back || maneuver || cavitating(noise, target.speed_kts, target.depth_m, pumpjet)?)
}
/// Keep the original fitted noise for ordinary propeller cavitation. Blow's
/// Loud projection must not manufacture a different propeller threshold.
fn passive_signature(
    w: &World,
    target: &crate::world::Entity,
    noise: usize,
    pumpjet: bool,
    airborne: bool,
) -> Result<i32, String> {
    let independent = !airborne && world_cavitating(w, target, noise, pumpjet)?;
    // Damage changes the detection signature, not the fitted propeller's
    // cavitation threshold. Query only after the raw-noise physical check.
    let effective_noise = super::signature_damage2026::project_noise(w, target.id, noise)?;
    let blow = super::navigation2026::submarine::emergency_blowing(w, target.id);
    if blow && !independent {
        return Err("3.2.1/5.4.6.5: Emergency Blow is passive Loud; Blow-only additional cavitation modifier combination pending adjudication".into());
    }
    Ok(i(&tables()["sonar"]["noise_modifiers"][if blow { 0 } else { effective_noise }])
        + if independent {
            i(&tables()["sonar"]["cavitation_noise_modifier"])
        } else {
            0
        })
}
pub fn sonar_bearing_eligible(mount: SonarMount, generation: usize, relative_bearing: f64) -> bool {
    let b = relative_bearing.rem_euclid(360.0);
    let a = angle_diff(b, 0.0).abs();
    match mount {
        SonarMount::Bow => a < 150.0,
        SonarMount::Keel | SonarMount::ForwardDigital => a < 135.0,
        SonarMount::Flank => generation >= 4 && (30.0..150.0).contains(&a),
        SonarMount::ForwardSailFixed => a <= 30.0,
        SonarMount::AftSailFixed => a >= 150.0,
        SonarMount::AftDigital => a > 45.0,
        _ => true,
    }
}
pub fn sonar_environment(active: bool, f: Frequency, env: &Environment) -> Result<i32, String> {
    if env.sea_state > 9
        || env.shipping > 3
        || env.biologics > 3
        || env.precipitation > 5
        || env.ice > 2
    {
        return Err("5.4: invalid environment".into());
    }
    let s = i(&tables()["sonar"][if active { "active_sea" } else { "passive_sea" }][env.sea_state]);
    let ice_m = if active {
        -2
    } else if f.idx() <= 1 {
        -2
    } else {
        -3
    };
    let mut total = match env.ice {
        0 => {
            if active {
                s
            } else {
                let mut noise = s - env.shipping as i32;
                if env.precipitation >= 2 {
                    noise = noise.min(i(&tables()["sonar"]["rain"][env.precipitation - 1]));
                }
                if env.biologics > 0 {
                    noise = noise.min(i(&tables()["sonar"][if f.idx() <= 1 {
                        "biologics_low"
                    } else {
                        "biologics_high"
                    }][env.biologics]));
                }
                noise
            }
        }
        1 => s + ice_m,
        2 => {
            if active {
                -3
            } else {
                3
            }
        }
        _ => unreachable!(),
    };
    if env.water_depth_m <= 200.0 {
        total += match f {
            Frequency::Vlf | Frequency::Lf | Frequency::Lmf => -3,
            Frequency::Mf => -2,
            Frequency::Hf => -1,
        }
    }
    Ok(total)
}
pub fn cz_count(
    active: bool,
    generation: usize,
    f: Frequency,
    noise: usize,
    target_kts: f64,
    searcher_kts: f64,
    mediterranean: bool,
) -> Result<usize, String> {
    if !(3..=7).contains(&generation) || noise > 4 {
        return Err("5.4.7.3: unsupported generation/noise".into());
    }
    if f == Frequency::Hf || (f == Frequency::Mf && !mediterranean) {
        return Ok(0);
    }
    let t = speed_col(target_kts)?;
    let s = speed_col(searcher_kts)?;
    let base = if active {
        i(&tables()["sonar"]["active_cz"][generation - 3])
    } else {
        i(&tables()["sonar"]["cz_base"][noise][3 - f.idx()])
    };
    Ok((base
        + if active {
            0
        } else {
            i(&tables()["sonar"]["cz_target_speed"][noise][t])
        }
        + i(&tables()["sonar"]["cz_searcher_speed"][s]))
    .max(0) as usize)
}
pub fn propagation_probability(
    total_modifiers: i32,
    generation: usize,
    cz_index: Option<usize>,
    alerted: bool,
) -> Result<f64, String> {
    if !(3..=7).contains(&generation) {
        return Err("5.4.7: unsupported generation".into());
    }
    if cz_index == Some(0) {
        return Err("5.4.7.3: CZ index begins at 1".into());
    }
    Ok((60
        + 5 * (total_modifiers + i(&tables()["sonar"]["propagation_generation"][generation - 3]))
        - 10 * cz_index.map(|c| c as i32 - 1).unwrap_or(0)
        + if alerted { 15 } else { 0 })
    .clamp(0, 90) as f64)
}
pub fn bottom_bounce_interval(
    depth_m: f64,
    rocky_flat: bool,
    frequency: Frequency,
) -> Result<Option<(f64, f64)>, String> {
    valid_nonnegative(depth_m)?;
    if !rocky_flat || frequency != Frequency::Lmf || !(2000.0..=5000.0).contains(&depth_m) {
        return Ok(None);
    }
    let row = tables()["sonar"]["bottom_bounce"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|r| n(&r[0]) <= depth_m)
        .unwrap();
    Ok(Some((n(&row[1]), n(&row[2]))))
}
pub fn cz_interval(
    index: usize,
    polar: bool,
    mediterranean: bool,
    water_depth_m: f64,
) -> Result<Option<(f64, f64)>, String> {
    valid_nonnegative(water_depth_m)?;
    if index == 0 {
        return Err("5.4.7.3: CZ index starts at 1".into());
    }
    if water_depth_m < 2100.0 {
        return Ok(None);
    }
    let center = index as f64
        * if mediterranean {
            20.0
        } else if polar {
            25.0
        } else {
            30.0
        };
    Ok(Some((center * 0.95, center * 1.05)))
}
/// Lower probability is masked; ties use closest contact. Equal ties do not
/// receive a fabricated arbitrary entity-id priority. Ordinary torpedoes are exempt.
pub fn passive_masked(
    bearing: f64,
    p: f64,
    range: f64,
    other_bearing: f64,
    other_p: f64,
    other_range: f64,
    torpedo_exempt: bool,
    same_path: bool,
) -> bool {
    !torpedo_exempt
        && same_path
        && angle_diff(bearing, other_bearing).abs() <= 5.0
        && (other_p > p || (other_p == p && other_range < range))
}
/// Classification level 0 none, 1 ship/sub/torpedo, 2 propulsion/nationality,
/// 3 true class/type, 4 positively identified mobile decoy.
pub fn sonar_classification(
    d100: u8,
    final_detection_chance: f64,
    f: Frequency,
    torpedo: bool,
    attack_speed: bool,
    decoy_generation: Option<usize>,
) -> Result<u8, String> {
    if !(1..=100).contains(&d100) {
        return Err("5.4.6.6: bad d100".into());
    }
    let narrow = f.idx() <= 1;
    if !torpedo && !narrow {
        return Ok(1);
    }
    let chance = final_detection_chance
        + 15.0
        + if torpedo {
            if narrow {
                30.0
            } else if attack_speed {
                20.0
            } else {
                10.0
            }
        } else {
            0.0
        };
    let r = d100 as f64;
    if let Some(g) = decoy_generation {
        let pen = match g {
            1 | 2 => 15.0,
            3 => 20.0,
            4 => 30.0,
            _ => return Err("unknown decoy generation".into()),
        };
        if r <= chance - pen {
            return Ok(4);
        }
    }
    Ok(if r <= chance {
        3
    } else if r <= chance + 10.0 {
        2
    } else if r <= chance + 20.0 {
        1
    } else {
        0
    })
}
pub fn ir_range(
    generation: usize,
    target_column: usize,
    precipitation: usize,
) -> Result<f64, String> {
    if !(1..=5).contains(&generation) || target_column > 2 || precipitation > 5 {
        return Err("5.5: missing IR generation/signature/environment".into());
    }
    Ok(
        n(&tables()["ir"]["ranges_nm"][generation - 1][target_column])
            * n(&tables()["ir"]["precipitation_multiplier"][precipitation]),
    )
}
pub fn laser_range(
    ship: bool,
    override_nm: Option<f64>,
    precipitation: usize,
) -> Result<f64, String> {
    if precipitation > 5 {
        return Err("5.6.1: unknown moisture".into());
    }
    let r = override_nm.unwrap_or(if ship { 10.0 } else { 5.0 });
    valid_nonnegative(r)?;
    Ok(r * n(&tables()["ir"]["precipitation_multiplier"][precipitation]))
}
pub fn mad_probability(
    size: usize,
    titanium: bool,
    depth: usize,
    masked: bool,
    poor: bool,
    localized: bool,
) -> Result<f64, String> {
    if size > 6 {
        return Err("5.7.1: invalid physical size".into());
    }
    if depth > 6 {
        return Ok(0.0);
    }
    let chance = 70
        + i(&tables()["mad"]["size_modifiers"][if titanium { 4 } else { size }])
        + i(&tables()["mad"]["depth_modifiers"][depth])
        - if masked { 30 } else { 0 }
        - if poor { 20 } else { 0 };
    Ok(chance.max(0) as f64 * if localized { 1.0 } else { 0.5 })
}
pub fn wide_mad_probability(
    area_nm2: f64,
    depth: usize,
    tactical_turns_inside: usize,
) -> Result<f64, String> {
    valid_nonnegative(area_nm2)?;
    if tactical_turns_inside > 10 {
        return Err("5.7.2: at most ten Tactical Turns per Intermediate Turn".into());
    }
    if depth > 6 {
        return Ok(0.0);
    }
    let limits = tables()["mad"]["wide_area_limits"].as_array().unwrap();
    let col = limits
        .iter()
        .position(|r| area_nm2 >= n(&r[0]) && area_nm2 <= n(&r[1]))
        .ok_or("5.7.2: area outside explicit table bins")?;
    Ok(
        n(&tables()["mad"]["wide_area_chance"][depth.saturating_sub(1)][col])
            * tactical_turns_inside as f64
            / 10.0,
    )
}
pub fn wake_probability(
    generation: usize,
    range_to_target: f64,
    target_speed: f64,
    observer_speed: f64,
    sea_state: usize,
    depth: usize,
    same_depth: bool,
    alerted: bool,
) -> Result<f64, String> {
    if !(1..=2).contains(&generation) || sea_state > 9 {
        return Err("5.7.3: unknown wake sensor/environment".into());
    }
    for v in [range_to_target, target_speed, observer_speed] {
        valid_nonnegative(v)?
    }
    if !same_depth || observer_speed >= 16.0 || sea_state >= 6 {
        return Ok(0.0);
    }
    let factor = if observer_speed > 8.0 { 0.5 } else { 1.0 }
        * if sea_state == 4 && depth <= 2 {
            0.5
        } else {
            1.0
        };
    for c in 0..3 {
        if range_to_target
            <= target_speed * n(&tables()["wake"]["time_late_hours"][generation - 1][c]) * factor
        {
            return Ok(
                (n(&tables()["wake"]["chance"][c]) + if alerted { 15.0 } else { 0.0 }).min(90.0),
            );
        }
    }
    Ok(0.0)
}

fn visibility_row(percent: u8) -> Result<usize, String> {
    tables()["visual"]["visibility"]
        .as_array()
        .unwrap()
        .iter()
        .position(|v| v.as_u64() == Some(percent as u64))
        .ok_or_else(|| "5.8: visibility must be an explicit printed category".into())
}
fn visual_size(size: usize) -> Result<usize, String> {
    Ok(match size {
        0..=4 => size,
        5 | 6 => 5,
        7 => 6,
        _ => return Err("5.8: physical size outside table".into()),
    })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VisualModifiers {
    pub cued: bool,
    pub maritime_patrol: bool,
    pub land_background: bool,
    pub narrow_small_craft: bool,
    pub night: bool,
    pub slow: bool,
    pub high_speed_wake: bool,
    pub missile_or_large_gun_flash: bool,
    pub small_gun_flash: bool,
    pub fire_or_smoke: bool,
    pub carrier_flight_ops: bool,
    pub group_size: usize,
    pub contrails: bool,
}
pub fn visual_base(
    visibility: u8,
    observer_size: usize,
    target_size: usize,
    observer_air: bool,
    target_air: bool,
    medium_search: bool,
    m: &VisualModifiers,
) -> Result<f64, String> {
    let mut row = visibility_row(visibility)?;
    if m.night {
        let increase = if m.missile_or_large_gun_flash || m.fire_or_smoke {
            3
        } else if m.small_gun_flash {
            2
        } else if m.high_speed_wake {
            1
        } else {
            0
        };
        if increase > 0 {
            row = row.saturating_sub(increase).max(3)
        }
        if m.carrier_flight_ops && visibility >= 60 && observer_air {
            row = row.saturating_sub(1)
        }
    }
    if m.slow {
        row = (row + 1).min(11)
    }
    if observer_air && !target_air && !medium_search {
        row = (row + 2).min(11)
    }
    let value = if target_air {
        let col = if target_size <= 1 { 1 } else { 0 };
        let group = if m.contrails {
            10.0
        } else if m.group_size >= 9 {
            3.0
        } else if m.group_size >= 3 {
            2.0
        } else {
            1.0
        };
        n(&tables()["visual"]["air_air"][row][col]) * group
    } else if observer_air {
        let col = if target_size == 7 {
            6
        } else {
            (target_size.min(4) / 2) * 2 + usize::from(medium_search)
        };
        tables()["visual"]["air_surface"][row][col]
            .as_f64()
            .ok_or("5.8.3: no Medium search in this visibility")?
    } else {
        n(
            &tables()["visual"]["surface"][row][visual_size(target_size)?]
                [visual_size(observer_size)?],
        )
    };
    Ok(if !m.night && m.fire_or_smoke && visibility >= 60 {
        (value * 2.0).min(52.0)
    } else {
        value
    })
}
/// Returns source kyds, and intentionally never silently rescales a source
/// row when the exact base is between rows (5.8.7.1 requires round DOWN).
pub fn visual_detection_range(base_kyd: f64, d100: u8, modifier: i32) -> Result<f64, String> {
    valid_nonnegative(base_kyd)?;
    if !(1..=100).contains(&d100) {
        return Err("5.8: invalid d100".into());
    }
    if base_kyd < 0.3 {
        return Ok(base_kyd);
    }
    let rows = tables()["visual"]["detection_rows"].as_array().unwrap();
    if base_kyd > 52.0 {
        return Err("5.8: modified base exceeds printed 52kyd detection table".into());
    }
    let row = rows
        .iter()
        .find(|r| n(&r[0]) <= base_kyd)
        .ok_or("5.8: base below minimum table row; automatic only below0.3kyd")?;
    let roll = (d100 as i32 + modifier).clamp(1, 100);
    let col = [10, 25, 50, 75, 100]
        .iter()
        .position(|v| roll <= *v)
        .unwrap();
    Ok(n(&row[col + 1]))
}
pub fn visual_roll_modifier(m: &VisualModifiers, target_size: usize) -> i32 {
    -20 * i32::from(m.cued) - 20 * i32::from(m.maritime_patrol)
        + if m.land_background {
            if m.night {
                if target_size <= 3 {
                    20
                } else {
                    30
                }
            } else if target_size <= 3 {
                10
            } else {
                20
            }
        } else {
            0
        }
        + if m.narrow_small_craft && target_size >= 5 {
            10
        } else {
            0
        }
}
pub fn periscope_visual_probability(
    sea_state: usize,
    speed_kts: f64,
    continuous: bool,
) -> Result<f64, String> {
    valid_nonnegative(speed_kts)?;
    if sea_state > 9 {
        return Err("5.8.5: invalid sea state".into());
    }
    if sea_state >= 7 {
        return Ok(0.0);
    }
    Ok(
        (n(&tables()["visual"]["periscope"][sea_state][usize::from(speed_kts >= 5.0)])
            - if continuous { 20.0 } else { 0.0 })
        .max(0.0),
    )
}
/// §5.8.7.1, reciprocal holes share one referee trial key, so neither side
/// gets a separate chance through the very same cloud in one Tactical Turn.
pub fn cloud_effect(
    state: &mut State,
    seed: u64,
    now: i64,
    a: EntityId,
    b: EntityId,
    alt_a: f64,
    alt_b: f64,
    cloud: Option<&Cloud>,
) -> Result<(bool, f64, Option<u8>), String> {
    let Some(c) = cloud else {
        return Ok((true, 1.0, None));
    };
    if ![0, 25, 50, 75, 100].contains(&c.coverage_percent)
        || !c.base_m.is_finite()
        || !c.top_m.is_finite()
        || c.base_m > c.top_m
    {
        return Err("5.8.7.1: invalid cloud layer".into());
    }
    if (alt_a < c.base_m && alt_b < c.base_m)
        || (alt_a > c.top_m && alt_b > c.top_m)
        || c.coverage_percent == 0
    {
        return Ok((true, 1.0, None));
    }
    let low = a.0.min(b.0);
    let high = a.0.max(b.0);
    let (roll, _) = trial(state, seed, now, &format!("5.8.cloud:{low}:{high}"), 180);
    Ok((
        roll <= 100 - c.coverage_percent,
        n(&tables()["visual"]["cloud_range_modifiers"][(c.coverage_percent / 25) as usize]),
        Some(roll),
    ))
}
pub fn land_visibility(
    base_kyd: f64,
    visibility: u8,
    camouflage: usize,
    fired: bool,
    infrared: bool,
) -> Result<f64, String> {
    valid_nonnegative(base_kyd)?;
    visibility_row(visibility)?;
    if camouflage > 3 {
        return Err("5.8.6: unknown camouflage".into());
    }
    if camouflage == 3 && !fired {
        return Ok(0.0);
    }
    let factor = if infrared {
        1.0
    } else {
        match camouflage {
            0 => 1.0,
            1 => 0.5,
            _ => 0.25,
        }
    };
    Ok(base_kyd * visibility as f64 / 100.0 * factor)
}
/// Sonobuoy localization, default conventional buoys. Optional specialized
/// EER/AQA/IEER branch is explicit, never inferred from buoy generation alone.
pub fn buoy_fix(
    active: bool,
    directional: bool,
    detecting_buoys: usize,
    special: &str,
) -> Result<&'static str, String> {
    let k = detecting_buoys;
    if k == 0 {
        return Ok("none");
    }
    let fix = match (active, directional, special) {
        (false, false, "") => {
            if k >= 2 {
                "poor"
            } else {
                "no_attack"
            }
        }
        (false, false, "aqa7") => {
            if k >= 3 {
                "fair"
            } else if k >= 2 {
                "poor"
            } else {
                "no_attack"
            }
        }
        (true, false, "") => {
            if k >= 3 {
                "good"
            } else if k >= 2 {
                "fair"
            } else {
                "no_attack"
            }
        }
        (true, false, "eer") => {
            if k >= 4 {
                "fair"
            } else if k >= 2 {
                "poor"
            } else {
                "no_attack"
            }
        }
        (false, true, "") => {
            if k >= 3 {
                "good"
            } else if k >= 2 {
                "poor"
            } else {
                "no_attack"
            }
        }
        (false, true, "eer") => {
            if k >= 3 {
                "good"
            } else if k >= 2 {
                "fair"
            } else {
                "no_attack"
            }
        }
        (true, true, "") | (true, true, "ieer") => {
            if k >= 2 {
                "good"
            } else {
                "fair"
            }
        }
        _ => return Err("5.4.6.3: unsupported buoy/processor combination".into()),
    };
    Ok(fix)
}
pub fn buoy_drop_valid(altitude_m: f64, speed_kts: f64) -> bool {
    (61.0..=7500.0).contains(&altitude_m) && speed_kts <= 325.0 && speed_kts >= 0.0
}
pub fn buoy_monitor_range(radar_horizon_nm: f64) -> Result<f64, String> {
    valid_nonnegative(radar_horizon_nm)?;
    Ok(radar_horizon_nm.min(40.0))
}
pub fn array_stabilization_seconds(long: bool, speed_kts: f64) -> Result<i64, String> {
    valid_nonnegative(speed_kts)?;
    if speed_kts < 5.0 {
        return Err("5.4.2.1 optional: stabilization below5kt not tabulated".into());
    }
    let col = if speed_kts >= 15.0 {
        2
    } else if speed_kts >= 10.0 {
        1
    } else {
        0
    };
    Ok(i(&tables()["sonar"]["array_stabilization_turns"][usize::from(long)][col]) as i64 * 180)
}
/// Explicit source capabilities; no speculative class inference.
fn project_target_coating(
    w: &World,
    e: &crate::world::Entity,
    mut profile: TargetProfile,
) -> Result<TargetProfile, String> {
    let anechoic = super::signature_damage2026::project_coating(
        w,
        e.id,
        profile.noise,
        profile.anechoic,
    )?;
    // Keep fitted noise in this common profile. Active sonar and reverse AIR
    // use effective coating; passive/CZ project noise separately so ordinary
    // cavitation keeps its original physical input.
    profile.anechoic = anechoic;
    Ok(profile)
}
fn target_profile(
    w: &World,
    config: &Configuration,
    e: &crate::world::Entity,
) -> Result<TargetProfile, String> {
    project_target_coating(w, e, raw_target_profile(w, config, e)?)
}
fn raw_target_profile(
    w: &World,
    config: &Configuration,
    e: &crate::world::Entity,
) -> Result<TargetProfile, String> {
    if let Some(p) = config
        .targets
        .get(&e.class)
        .filter(|p| !p.source.trim().is_empty())
    {
        return Ok(p.clone());
    }
    if let Some(p)=super::surface2026::antiship::incoming_detection_profile(w,&e.class)?{return Ok(p);}
    let ops = super::operations::tables();
    let size = if e.kind == PlatformKind::Aircraft {
        super::aviation2026::operations_aircraft(w, &e.class)
            .and_then(|a| a["size"].as_u64())
            .map(|s| match s {
                0 => 4,
                1 => 2,
                2 => 1,
                _ => 0,
            })
    } else {
        ops["radar"]["physical_sizes"][&e.class]
            .as_str()
            .and_then(|s| s.as_bytes().first().copied())
            .map(|b| (b - b'A') as u64)
    };
    let Some(size) = size else {
        return Err("chapter5: target physical size not source-verified".into());
    };
    Ok(TargetProfile {
        source: "operations-v3 verified physical size".into(),
        size: size as usize,
        noise: None,
        anechoic: false,
        titanium: false,
        pumpjet: false,
        stealthy_ir: false,
        radar_signature: None,
    })
}
fn sensed_profile(
    w: &World,
    config: &Configuration,
    e: &crate::world::Entity,
    acoustic_coating: bool,
) -> Result<TargetProfile, String> {
    if let Some(object) = super::ew2026::object(w, e.id) {
        return match object.kind {
            super::ew2026::ObjectKind::MobileDecoy => object
                .imitated_profile
                .clone()
                .ok_or("12.6.2: missing saved imitated acoustic signature".into()),
            super::ew2026::ObjectKind::SonarJammer => Ok(TargetProfile {
                source: "English2026 12.6.1 Loud sonar jammer; no physical size claim".into(),
                size: usize::MAX,
                noise: Some(0),
                anechoic: false,
                titanium: false,
                pumpjet: false,
                stealthy_ir: false,
                radar_signature: None,
            }),
            _ => Err(
                "chapter12: acoustic/visual physical profile of RF-only decoy is not specified"
                    .into(),
            ),
        };
    }
    if acoustic_coating {
        target_profile(w, config, e)
    } else {
        raw_target_profile(w, config, e)
    }
}
/// Real World adapter, called from sensors::update_sensors only for v4.
/// A blocked source fit is Err, never a fallback to the old estimated detector.
/// Radar uses operations::radar_range and its timers; this adapter adds mast
/// fleeting trials. Group-signature range promotion requires parent range hook.
pub fn evaluate_world(
    w: &World,
    state: &mut State,
    observer: EntityId,
    sensor: &str,
    target: EntityId,
) -> Result<ScanDecision, String> {
    let o = w.entity(observer).ok_or("observer absent")?;
    let t = w.entity(target).ok_or("target absent")?;
    if !o.alive || !t.alive {
        return Ok(ScanDecision::new(
            false,
            true,
            json!({"rule":"5.1","reason":"inactive"}),
        ));
    }
    if crate::sensors::sensor_damaged(o, sensor) {
        return Ok(ScanDecision::new(
            false,
            true,
            json!({"rule":"5.1","reason":"sensor_damaged"}),
        ));
    }
    if !super::expanded_damage2026::sensor_operational(w,observer,sensor)? {
        return Ok(ScanDecision::new(false,true,json!({"rule":"14.5","reason":"source physical sensor component unavailable"})));
    }
    let def = w.db.sensors.get(sensor).ok_or("sensor absent")?;
    if let Some(object) = super::ew2026::object(w, target) {
        let allowed = match object.kind {
            super::ew2026::ObjectKind::LongRangeChaff | super::ew2026::ObjectKind::FloatingAsm => {
                def.kind == crate::db::SensorKind::Radar
            }
            super::ew2026::ObjectKind::SonarJammer => {
                def.kind == crate::db::SensorKind::SonarPassive
            }
            super::ew2026::ObjectKind::MobileDecoy => match def.kind {
                crate::db::SensorKind::SonarPassive => true,
                crate::db::SensorKind::SonarActive => object.generation.is_some_and(|g| g >= 3),
                crate::db::SensorKind::Magnetic => super::ew2026::mobile_mad_capable(object.generation)?,
                _ => false,
            },
            _ => false,
        };
        if !allowed {
            return Ok(ScanDecision::new(
                false,
                true,
                json!({"rule":"12.sensor_return","reason":"source_decoy_has_no_return_in_this_sensor_domain"}),
            ));
        }
    }
    if def.kind != crate::db::SensorKind::Visual
        && crate::rules::damage_runtime::marker(o)
        && o.ext.get("damage.power_lost").copied().unwrap_or(0.0) > 0.0
    {
        return Ok(ScanDecision::new(
            false,
            true,
            json!({"rule":"5.1","reason":"power_lost"}),
        ));
    }
    if matches!(
        def.kind,
        crate::db::SensorKind::SonarActive | crate::db::SensorKind::SonarPassive
    ) && o.systems.sonar <= 0.05
    {
        return Ok(ScanDecision::new(
            false,
            true,
            json!({"rule":"5.4","reason":"sonar_inoperative"}),
        ));
    }
    if def.kind == crate::db::SensorKind::Radar {
        let range = radar_range_world(w, state, o, def, t)?;
        let mut d = ScanDecision::new(
            range > 0.0 && o.pos.range_to(t.pos) <= range,
            false,
            json!({"rule":"5.2","range_nm":range}),
        );
        if t.kind == PlatformKind::Sub && t.depth_m > 0.0 {
            let mast = effective_mast(w,state,target)?
                .ok_or("5.2.1: explicit raised mast state required")?;
            if mast.count == 0 || t.depth_m > 25.0 || t.speed_kts > 15.0 {
                d.detected = false;
                return Ok(d);
            }
            let env = state
                .config
                .environment
                .as_ref()
                .ok_or("5.2.1: missing sea state")?;
            let profile = super::aviation2026::operations_radar_profile(w, o, def)
                .ok_or("radar fit unverified")?;
            let p = radar_mast_probability(
                env.sea_state,
                profile["isar"].as_bool().unwrap_or(false)
                    || matches!(
                        state.config.bindings.get(&format!("{}:{sensor}", o.class)),
                        Some(Receiver::RadarFeatures { isar: true, .. })
                    ),
                w.tick - mast.raised_since,
            )?;
            let (roll, reused) = trial(
                state,
                w.seed,
                w.tick,
                &format!("5.2.mast:{}:{sensor}:{}", observer.0, target.0),
                180,
            );
            d.detected &= roll <= p;
            d.trace = json!({"rule":"5.2.1","range_nm":range,"probability":p,"d100":roll,"reused":reused});
        } else {
            d.detected &= super::operations::radar_recognized(w, o, def, t);
        }
        d.range_known = d.detected;
        if d.detected && !mast_contact(t) {
            d.known_category = Some(t.combat_kind());
        }
        if let Some(Receiver::RadarFeatures {
            sar,
            isar,
            nctr,
            source,
        }) = state
            .config
            .bindings
            .get(&format!("{}:{sensor}", o.class))
            .filter(|r| !r.source().trim().is_empty())
        {
            d.identify_class = d.detected && radar_identifies(*sar, *isar, *nctr, t.combat_kind());
            d.trace["capability_source"] = json!(source);
        }
        if super::ew2026::is_object(w, target) {
            d.identify_class = false;
        }
        if t.kind == PlatformKind::Aircraft && d.detected {
            if let Some((group, members)) = state.groups.iter().find(|(_, members)| {
                members.contains(&target)
                    && members
                        .iter()
                        .filter(|id| w.entity(**id).is_some_and(|e| e.alive))
                        .count()
                        >= 3
            }) {
                let _ = members;
                d.group = Some(group.clone());
            }
        }
        return Ok(d);
    }
    let env = state
        .config
        .environment
        .clone()
        .ok_or("chapter5: missing explicit detection environment")?;
    env.validate()?;
    if env.source.trim().is_empty() {
        return Err("chapter5: environment provenance absent".into());
    }
    let receiver = state
        .config
        .bindings
        .get(&format!("{}:{sensor}", o.class))
        .cloned()
        .or_else(|| {
            // Human lookouts are a universal rule, not an estimated annex sensor.
            (def.kind == crate::db::SensorKind::Visual).then(|| Receiver::Visual {
                source: "English2026 5.8 human lookout".into(),
            })
        })
        .ok_or("chapter5: fitted receiver capability not source-verified")?;
    if receiver.source().trim().is_empty() {
        return Err("chapter5: receiver source absent".into());
    }
    if matches!(&receiver,Receiver::Sonar{..}) {
        super::asw_weapons2026::projector::attack_sonar::scan_gate(w,observer,sensor,target,&receiver)?;
    }
    let range = o.pos.range_to(t.pos);
    let key = format!("{}:{sensor}:{}", observer.0, target.0);
    let track = state.tracks.get(&key).cloned().unwrap_or_default();
    let held = track.held;
    let alerted = if matches!(&receiver,Receiver::Sonar{..}) {
        ordinary_sonar_alerted(w,state,observer,target)||super::communications2026::sonar_alert(w,observer,target)
    } else {track.last_success.is_some_and(|s|w.tick-s<=720)};
    let probability: f64;
    let rule: &str;
    let (mut bearing_only, mut class, mut fix, mut error) = (true, false, None, None);
    let mut trace = json!({});
    let p = match &receiver {
        Receiver::Es { .. } | Receiver::AcousticIntercept { .. } => None,
        _ => Some(sensed_profile(
            w,
            &state.config,
            t,
            matches!(&receiver, Receiver::Sonar { .. }),
        )?),
    };
    let receiver_info = receiver.clone();
    match receiver {
        Receiver::RadarFeatures { .. } => {
            return Err("5.2: radar capability binding on nonradar sensor".into());
        }
        Receiver::Tv {
            low_light,
            range_nm,
            arc_deg,
            ..
        } => {
            let visible = if low_light {
                env.visibility_percent >= 5 && env.precipitation == 0
            } else {
                env.visibility_percent >= 30
            };
            let cued = w.sides[o.side.index()]
                .contacts
                .values()
                .any(|c| c.about == target && !c.stale);
            let arc = angle_diff(o.course_deg, o.pos.bearing_to(t.pos)).abs() <= arc_deg / 2.0;
            let (gap, _, _) = cloud_effect(
                state,
                w.seed,
                w.tick,
                observer,
                target,
                o.altitude_m,
                t.altitude_m,
                env.cloud.as_ref(),
            )?;
            let detected = visible && cued && arc && gap && range <= range_nm;
            let mut r = ScanDecision::new(
                detected,
                true,
                json!({"rule":"5.5.4","cued":cued,"low_light":low_light,"visibility_eligible":visible}),
            );
            r.identify_class = detected;
            r.known_category = if detected {
                Some(t.combat_kind())
            } else {
                None
            };
            return Ok(r);
        }
        Receiver::Visual { .. } => {
            let tp = p.as_ref().unwrap();
            let op = raw_target_profile(w, &state.config, o)?;
            if t.kind == PlatformKind::Sub && t.depth_m > 25.0 {
                return Ok(ScanDecision::new(
                    false,
                    true,
                    json!({"rule":"5.8.5","reason":"submerged"}),
                ));
            }
            if o.kind == PlatformKind::Aircraft && o.altitude_m > 7500.0 {
                return Err("5.8.3: search altitude must be Low or Medium".into());
            }
            let group = state
                .groups
                .values()
                .find(|g| g.contains(&target))
                .map_or(1, |g| {
                    g.iter()
                        .filter(|id| {
                            w.entity(**id)
                                .is_some_and(|e| e.alive && e.kind == PlatformKind::Aircraft)
                        })
                        .count()
                });
            let mods = VisualModifiers {
                night: w.day_night == crate::types::DayNight::Night,
                slow: t.kind == PlatformKind::Ship && t.speed_kts <= 5.0,
                high_speed_wake: t.kind == PlatformKind::Ship && t.speed_kts >= 20.0,
                fire_or_smoke: t.kind == PlatformKind::Ship && t.burn_s > 0.0,
                group_size: group,
                ..Default::default()
            };
            let visibility = if let Some(squalls) = &state.squalls { squalls::visibility(squalls, w.tick, o.pos, t.pos, env.visibility_percent)? } else { env.visibility_percent };
            let mut base = visual_base(
                visibility,
                op.size,
                tp.size,
                o.kind == PlatformKind::Aircraft,
                t.kind == PlatformKind::Aircraft,
                o.altitude_m > 2000.0,
                &mods,
            )?;
            if o.kind == PlatformKind::Base {
                let height = *state
                    .config
                    .target_heights_m
                    .get(&o.class)
                    .ok_or("5.8.6: observation post eye height required")?;
                let column=[8.0,15.0,30.0,45.0,60.0].iter().position(|h|(*h-height).abs()<1e-9).ok_or("5.8.6: observation eye height outside printed table; interpolation unspecified")?;
                base = n(&tables()["visual"]["land_observer"][visual_size(tp.size)?][column])
                    * visibility as f64
                    / 100.0;
            }
            if t.kind == PlatformKind::Base {
                if tp.size < 2 || tp.size > 7 {
                    return Err(
                        "5.8.6: land structure must have explicit C–H physical class".into(),
                    );
                }
                let bare =
                    n(&tables()["visual"]["land_target"][tp.size - 2][visual_size(op.size)?]);
                let camouflage = state.config.camouflage.get(&t.class).copied().unwrap_or(0);
                let fired = state.fired.contains(&target);
                base = land_visibility(bare, visibility, camouflage, fired, false)?;
            }
            if t.kind == PlatformKind::Sub && t.depth_m > 0.0 {
                if let Some(m) = effective_mast(w,state,target)? {
                    if m.count >= 2 {
                        base *= 1.5;
                    }
                }
            }
            let (gap, cloud_mult, cloud_die) = cloud_effect(
                state,
                w.seed,
                w.tick,
                observer,
                target,
                o.altitude_m,
                t.altitude_m,
                env.cloud.as_ref(),
            )?;
            base *= cloud_mult;
            let visual_key = format!("5.8.visual:{}:{sensor}", observer.0); // same lookout roll for all units of a formation
            let (roll, reused) = trial(state, w.seed, w.tick, &visual_key, 180);
            let visible_range =
                visual_detection_range(base, roll, visual_roll_modifier(&mods, tp.size))?;
            let in_range = range * 2025.3718285 / 1000.0 <= if held { base } else { visible_range };
            let blinded=super::surface2026::gunnery::starshell_blinds(w,observer,target);
            let mut detected = gap && in_range && !blinded;
            if t.kind == PlatformKind::Sub && t.depth_m > 0.0 {
                let mast = effective_mast(w,state,target)?
                    .ok_or("5.8.5: missing raised mast state")?;
                if mast.count == 0 {
                    detected = false
                }
                let chance = periscope_visual_probability(
                    env.sea_state,
                    t.speed_kts,
                    w.tick - mast.raised_since >= 180,
                )?;
                let (mast_roll, _) = trial(state, w.seed, w.tick, &format!("5.8.mast:{key}"), 180);
                detected &= (mast_roll as f64) <= chance;
                trace = json!({"mast_chance":chance,"mast_d100":mast_roll});
            }
            let mut result = ScanDecision::new(
                detected,
                false,
                json!({"rule":"5.8","base_kyd":base,"detection_kyd":visible_range,"d100":roll,"reused":reused,"cloud_d100":cloud_die,"cloud_gap":gap,"mast":trace}),
            );
            // Basic5.8.9 immediate visual class ID; optional graded ID is separate.
            result.identify_class = detected;
            result.known_category = if detected {
                Some(t.combat_kind())
            } else {
                None
            };
            state.tracks.entry(key).or_default().held = detected;
            return Ok(result);
        }
        Receiver::Ir {
            generation,
            mode,
            arc_deg,
            ..
        } => {
            if !state.config.targets.contains_key(&t.class) {
                return Err("5.5: explicit IR target signature/profile required".into());
            }
            let tp = p.as_ref().unwrap();
            let supersonic = if matches!(t.kind, PlatformKind::Aircraft | PlatformKind::Missile)
                && !tp.stealthy_ir
                && tp.size > 1
            {
                state.supersonic.get(&target).copied().ok_or("5.5: explicit sonic regime required; loadout speed limits are not speed of sound")?
            } else {
                false
            };
            let col = if tp.stealthy_ir {
                0
            } else if tp.size <= 1 || supersonic {
                2
            } else {
                1
            };
            let r = ir_range(generation, col, env.precipitation)?;
            let rear = angle_diff(t.course_deg + 180.0, t.pos.bearing_to(o.pos)).abs() <= 60.0;
            let arc = angle_diff(o.course_deg, o.pos.bearing_to(t.pos)).abs() <= arc_deg / 2.0;
            let cued = w.sides[o.side.index()]
                .contacts
                .values()
                .any(|c| c.about == target && !c.stale);
            let eligible = range <= r
                && arc
                && (mode != IrMode::Irst || generation > 1 || rear)
                && (mode != IrMode::Flir || cued);
            if !eligible {
                state.tracks.entry(key).or_default().held = false;
                return Ok(ScanDecision::new(
                    false,
                    true,
                    json!({"rule":"5.5","range_nm":r,"reason":"outside_envelope_or_not_cued"}),
                ));
            }
            probability = if held { 100.0 } else { 80.0 };
            rule = "5.5";
            class = range <= r / 2.0 && (mode != IrMode::Irst || generation >= 3);
            trace = json!({"range_nm":r,"retained":held});
        }
        Receiver::Es {
            generation,
            targeting_complex,
            rwr,
            bearing_error_deg,
            els,
            ..
        } => {
            if let Some(config) = &w.context.hf_es_2026 {
                let sample = super::communications2026::hf_es::sample(w, config, observer)?
                    .into_iter().find(|s|s.sensor==sensor && s.emitter==target && s.emitted_at==w.tick);
                if let Some(sample) = sample {
                    if let Some(pulse) = super::communications2026::hf_es::es_sources(w)
                        .into_iter().find(|p|p.emitter==target && p.at==sample.emitted_at) {
                        let mut result = ScanDecision::new(true,true,json!({
                            "rule":"6.3.9.hf_es","reference":sample.reference,
                            "emitted_at":sample.emitted_at,"receiver_source":sample.source,
                            "policy":"actual committed current-tick pulse; authored deterministic reception; no historical-pulse detection, fix, category or FCS"}));
                        result.source_point=Some(pulse.position);
                        result.bearing_error_deg=Some(sample.bearing_error_deg);
                        return Ok(result);
                    }
                }
            }
            let mut emission = state
                .emissions
                .get(&target)
                .cloned()
                .ok_or("5.3: explicit active/search/LPI emitter metadata required")?;
            // Scenario metadata supplies characteristics, never the emission
            // switch. Re-read physical state after every actual radar command
            // or casualty. Old referee-only mechanism inputs remain supported.
            if let Some(binding) = state.config.emissions.get(&t.name) {
                emission.emitting = emitter_hardware_operating(w, target, &binding.sensor);
            }
            if !emission.emitting || (rwr && !emission.weapon_radar) {
                return Ok(ScanDecision::new(
                    false,
                    true,
                    json!({"rule":"5.3","reason":"not_emitting_or_RWR_search"}),
                ));
            }
            let h = if let Some(duct) = &state.config.duct {
                duct_horizon(duct.surface, duct_size(w, o)?, duct_size(w, t)?)?
            } else {
                crate::geom::horizon_nm(
                    super::operations::radar_height(o)?,
                    emission.antenna_height_m,
                )
            };
            let r = es_range(
                h,
                generation,
                targeting_complex,
                &emission.radar_role,
                emission.max_range_nm,
                emission.lpi,
            )?;
            if range > r {
                return Ok(ScanDecision::new(
                    false,
                    true,
                    json!({"rule":"5.3","range_nm":r}),
                ));
            }
            probability = if emission.weapon_radar { 100.0 } else { 90.0 };
            rule = "5.3";
            error = if rwr && generation <= 1 {
                Some(if generation == 0 { 45.0 } else { 20.0 })
            } else if targeting_complex {
                Some(0.5)
            } else {
                bearing_error_deg
            };
            if error.is_none() {
                return Err("5.3: ES bearing resolution not specified; no guessed fix".into());
            }
            // ELS never reveals truth position; parent must derive a public fix from bearings.
            if (els || generation >= 5)
                && o.kind == PlatformKind::Aircraft
                && t.kind != PlatformKind::Aircraft
            {
                fix = Some("els_candidate".into())
            }
            trace = json!({"range_nm":r,"emitter_type":emission.radar_role,"lpi":emission.lpi});
        }
        Receiver::Sonar {
            generation,
            passive_range_nm,
            active_range_nm,
            frequency,
            mount,
            active_torpedo_detection,
            high_speed_tow,
            ..
        } => {
            let tp = p.as_ref().unwrap();
            let mut env=env.clone();
            let nuclear_blueout=super::nuclear2026::blueout_active(w,observer,sensor);
            if nuclear_blueout {env.sea_state=super::nuclear2026::tables()["rules"]["blueout_sea_state"].as_u64().unwrap() as usize;}
            if matches!(
                mount,
                SonarMount::Towed | SonarMount::Vds | SonarMount::Dipping
            ) {
                if !state
                    .arrays
                    .get(&format!("{}:{sensor}", observer.0))
                    .is_some_and(|a| a.deployed && !a.lost)
                {
                    return Err("5.4.2/5.4.3: array not fully deployed or lost".into());
                }
            }
            let active = def.kind == crate::db::SensorKind::SonarActive
                && o.sensors.iter().any(|s| s.sensor == sensor && s.active);
            let airborne = t.kind == PlatformKind::Aircraft;
            if airborne
                && (active || frequency.idx() > 1 || t.altitude_m > 2000.0 || o.speed_kts > 8.0)
            {
                return Ok(ScanDecision::new(
                    false,
                    true,
                    json!({"rule":"5.4.6.4","reason":"airborne acoustic source outside VLF/LF-Low/searcher8kt envelope"}),
                ));
            }
            if matches!(t.kind, PlatformKind::Missile | PlatformKind::Base) {
                return Err("5.4.6: unsupported land/missile acoustic source".into());
            }
            let active = def.kind == crate::db::SensorKind::SonarActive
                && o.sensors.iter().any(|s| s.sensor == sensor && s.active);
            let mode_key = format!("{}:{}", observer.0, receiver_info.source());
            let window = w.tick.div_euclid(180);
            if generation <= 4 {
                if state
                    .modes
                    .get(&mode_key)
                    .is_some_and(|(win, chosen)| *win == window && *chosen != active)
                {
                    return Err("5.4.6: Gen1-4 sonar mode locked for current Tactical Turn".into());
                }
                state.modes.insert(mode_key, (window, active));
            }
            if !sonar_bearing_eligible(
                mount,
                generation,
                angle_diff(o.course_deg, o.pos.bearing_to(t.pos)),
            ) {
                return Ok(ScanDecision::new(
                    false,
                    true,
                    json!({"rule":"5.4.1","reason":"baffles"}),
                ));
            }
            if active && t.kind == PlatformKind::Torpedo && !active_torpedo_detection {
                return Ok(ScanDecision::new(
                    false,
                    true,
                    json!({"rule":"5.4.6.1","reason":"cannot_detect_torpedo"}),
                ));
            }
            if mount == SonarMount::Dipping
                && o.speed_kts > n(&tables()["sonar"]["dipping"]["max_horizontal_speed_kts"])
            {
                return Err(
                    "5.4.3: moving platform loses dipping sonar; lifecycle hook required".into(),
                );
            }
            let base = if active {
                active_range_nm
            } else {
                passive_range_nm
            }
            .ok_or("5.4.6: selected sonar mode lacks verified50%range")?;
            let noise = if active {
                0
            } else if airborne {
                3
            } else {
                tp.noise
                    .ok_or("5.4.6: source target noise rating required")?
            };
            if noise > 4 || (active && tp.size > 6) {
                return Err("5.4.6: invalid target signature".into());
            }
            let Some(relative) = (if airborne {
                Some(0)
            } else {
                sonar_speed_modifier(active, t.speed_kts, o.speed_kts, high_speed_tow)?
            }) else {
                return Ok(ScanDecision::new(
                    false,
                    true,
                    json!({"rule":"5.4.6","reason":"own_speed_blind"}),
                ));
            };
            let mut target_mod = if active {
                i(&tables()["sonar"]["active_strength"][tp.size]) - i32::from(tp.anechoic)
                    + active_aspect_modifier(angle_diff(t.course_deg, t.pos.bearing_to(o.pos)))
            } else {
                passive_signature(w,t,noise,tp.pumpjet,airborne)?
            };
            if !active {
                target_mod = passive_source_modifier(w, state, t, frequency, target_mod)?;
            }
            let environment = sonar_environment(active, frequency, &env)?;
            // Towed sensor depth is not approximated by parent platform depth.
            let sonar_depth = deployed_sonar_depth(state, o, sensor, mount)?;
            let layer = layer_modifier(frequency, !active, sonar_depth, t.depth_m, env.layer)?;
            let total = target_mod + relative + environment + layer;
            let modified = base * sonar_range_modifier(total)?;
            let path = state
                .config
                .propagation
                .get(&format!("{}:{sensor}", o.class))
                .cloned()
                .unwrap_or(Propagation::Direct);
            let own_chance = match path {
                Propagation::Direct => sonar_probability(modified, range, alerted)?,
                Propagation::BottomBounce { rocky_flat } => {
                    if !active {
                        return Err("5.4.7.2: bottom bounce requires active LMF sonar".into());
                    }
                    if bottom_bounce_interval(env.water_depth_m, rocky_flat, frequency)?
                        .is_some_and(|(a, b)| range >= a && range <= b)
                    {
                        propagation_probability(
                            target_mod + relative + environment,
                            generation,
                            None,
                            alerted,
                        )?
                    } else {
                        0.0
                    }
                }
                Propagation::ConvergenceZone {
                    polar,
                    mediterranean,
                } => {
                    if nuclear_blueout {return Ok(ScanDecision::new(false,true,json!({"rule":"5.4.6.5","reason":"CZ unavailable during source nuclear blue-out"})));}
                    let count = cz_count(
                        active,
                        generation,
                        frequency,
                        if active {
                            noise
                        } else if super::navigation2026::submarine::emergency_blowing(w, t.id) {
                            0
                        } else {
                            super::signature_damage2026::project_noise(w, t.id, noise)?
                        },
                        t.speed_kts,
                        o.speed_kts,
                        mediterranean,
                    )?;
                    let zone = (1..=count).find(|z| {
                        cz_interval(*z, polar, mediterranean, env.water_depth_m)
                            .ok()
                            .flatten()
                            .is_some_and(|(a, b)| range >= a && range <= b)
                    });
                    if let Some(z) = zone {
                        propagation_probability(
                            target_mod + relative + environment,
                            if active { 5 } else { generation },
                            Some(z),
                            alerted,
                        )?
                    } else {
                        0.0
                    }
                }
            };
            let jammer_target = super::ew2026::object(w, target)
                .is_some_and(|j| j.kind == super::ew2026::ObjectKind::SonarJammer);
            let jammer_half = super::ew2026::tables()["acoustic_countermeasures"]
                ["sonar_jammer_passive_cone_total_deg"]
                .as_f64()
                .unwrap()
                / 2.;
            let jammed = !active
                && !jammer_target
                && w.entities
                    .values()
                    .filter(|other| {
                        other.id != target
                            && super::ew2026::object(w, other.id).is_some_and(|j| {
                                j.kind == super::ew2026::ObjectKind::SonarJammer
                                    && j.effective(w.tick)
                            })
                    })
                    .any(|jammer| {
                        angle_diff(o.pos.bearing_to(t.pos), o.pos.bearing_to(jammer.pos)).abs()
                            <= jammer_half
                            && passive_strength(w, state, o, jammer, sensor, &receiver_info)
                                .is_ok_and(|p| p > 0.)
                    });
            let masked = if !active && t.kind != PlatformKind::Torpedo {
                w.entities
                    .values()
                    .filter(|other| {
                        other.alive
                            && other.id != target
                            && other.id != observer
                            && other.kind != PlatformKind::Torpedo
                            // §12.6.1's dedicated cone leaves the jammer audible;
                            // the contact it suppresses cannot mask it in return.
                            // Other jammers still use the normal strength/tie rule.
                            && (!jammer_target || super::ew2026::object(w,other.id).is_some_and(|j|j.kind==super::ew2026::ObjectKind::SonarJammer))
                    })
                    .try_fold(false, |masked,other| -> Result<bool,String> {
                        let strength=passive_strength(w,state,o,other,sensor,&receiver_info);
                        if let Err(reason)=&strength {
                            if super::navigation2026::submarine::emergency_blowing(w,other.id)
                                && angle_diff(o.pos.bearing_to(t.pos),o.pos.bearing_to(other.pos)).abs()<=5. {
                                return Err(format!("3.2.1/5.4.6 unresolved Emergency Blow masking source: {reason}"));
                            }
                        }
                        Ok(masked || strength.is_ok_and(
                            |p| {
                                passive_masked(
                                    o.pos.bearing_to(t.pos),
                                    own_chance,
                                    range,
                                    o.pos.bearing_to(other.pos),
                                    p,
                                    o.pos.range_to(other.pos),
                                    false,
                                    true,
                                )
                            },
                        ))
                    })?
            } else {
                false
            };
            probability = if masked || jammed { 0.0 } else { own_chance };
            bearing_only = !active;
            rule = "5.4.6";
            trace = json!({"base50_nm":base,"target_modifier":target_mod,"relative_speed":relative,"environment":environment,"layer":layer,"total":total,"modified50_nm":modified,"alerted":alerted,"frequency":frequency,"mode_active":active,"sonar_jammed":jammed});
        }
        Receiver::AcousticIntercept { generation, .. } => {
            let op = target_profile(w, &state.config, o)?;
            if op.size > 6 {
                return Err("5.4.5: listener strength unverified".into());
            }
            let mut best = None;
            // §2.5.1 committed recall signal is a real LMF active emitter.
            // AIR uses its source5nm active base, never passive noise +8.
            for emission in super::unmanned2026::acoustic_command_sources(w).into_iter().filter(|s|s.emitter==target) {
                let Some(speed)=sonar_speed_modifier(true,o.speed_kts,t.speed_kts,false)? else {continue};
                let total=i(&tables()["sonar"]["active_strength"][op.size])-i32::from(op.anechoic)
                    +active_aspect_modifier(angle_diff(o.course_deg,o.pos.bearing_to(t.pos)))+speed
                    +sonar_environment(true,emission.frequency,&env)?
                    +layer_modifier(emission.frequency,false,t.depth_m,o.depth_m,env.layer)?;
                let modified=emission.base50_nm*sonar_range_modifier(total)?;
                let hit=acoustic_intercept(generation,emission.frequency,modified,range,true)?;
                if hit.0 {best=Some((hit,"2.5.1.public_LMF_command".into(),modified));break;}
            }
            for inst in &t.sensors {
                if !inst.active
                    || crate::sensors::sensor_damaged(t, &inst.sensor)
                    || !w
                        .db
                        .sensors
                        .get(&inst.sensor)
                        .is_some_and(|d| d.kind == crate::db::SensorKind::SonarActive)
                    || t.systems.sonar <= 0.05
                    || (crate::rules::damage_runtime::marker(t)
                        && t.ext.get("damage.power_lost").copied().unwrap_or(0.0) > 0.0)
                {
                    continue;
                }
                let Some(Receiver::Sonar {
                    active_range_nm: Some(base),
                    frequency,
                    mount,
                    ..
                }) = state
                    .config
                    .bindings
                    .get(&format!("{}:{}", t.class, inst.sensor))
                else {
                    continue;
                };
                let Ok(sd) = deployed_sonar_depth(state, t, &inst.sensor, *mount) else {
                    continue;
                };
                let Some(speed) = sonar_speed_modifier(true, o.speed_kts, t.speed_kts, false)?
                else {
                    continue;
                };
                let total = i(&tables()["sonar"]["active_strength"][op.size])
                    - i32::from(op.anechoic)
                    + active_aspect_modifier(angle_diff(o.course_deg, o.pos.bearing_to(t.pos)))
                    + speed
                    + sonar_environment(true, *frequency, &env)?
                    + layer_modifier(*frequency, false, sd, o.depth_m, env.layer)?;
                let modified = base * sonar_range_modifier(total)?;
                let direct = matches!(
                    state
                        .config
                        .propagation
                        .get(&format!("{}:{}", t.class, inst.sensor)),
                    None | Some(Propagation::Direct)
                );
                if !direct {
                    return Err("5.4.5: indirect-path AIR is a bearingless warning; public contact representation pending, no invented bearing".into());
                }
                let hit = acoustic_intercept(generation, *frequency, modified, range, direct)?;
                if hit.0 {
                    best = Some((hit, inst.sensor.clone(), modified));
                    break;
                }
            }
            let Some((hit, emitter, modified)) = best else {
                return Ok(ScanDecision::new(
                    false,
                    true,
                    json!({"rule":"5.4.5","reason":"no_eligible_active_emission"}),
                ));
            };
            let mut result = ScanDecision::new(
                true,
                !hit.3,
                json!({"rule":"5.4.5","emitter_type_known":hit.2,"sensor":if hit.2{Some(emitter)}else{None},"modified50_nm":modified}),
            );
            result.bearing_error_deg = hit.1;
            if t.kind == PlatformKind::Torpedo {
                result.known_category = Some(PlatformKind::Torpedo);
                // §5.4.6.6: an active torpedo seeker heard by AIR is
                // automatically classified. This does not reveal its type.
                result.trace["automatic_torpedo_classification"] = json!(true);
            }
            return Ok(result);
        }
        Receiver::Mad {
            range_nm, western, ..
        } => {
            let tp = p.as_ref().unwrap();
            let band = depth_band(t.depth_m)?;
            let localized = w.sides[o.side.index()]
                .contacts
                .values()
                .any(|c| c.about == target && !c.stale && !c.bearing_only);
            if t.kind != PlatformKind::Sub
                || !(61.0..=2000.0).contains(&o.altitude_m)
                || range > range_nm
                || (western && !localized)
            {
                return Ok(ScanDecision::new(
                    false,
                    true,
                    json!({"rule":"5.7.1","reason":"outside_MAD_pass"}),
                ));
            }
            if angle_diff(o.course_deg, o.ordered_course_deg).abs() > 0.1
                || (o.altitude_m - o.ordered_altitude_m).abs() > 1.0
            {
                return Err("5.7.1: MAD requires straight level pass".into());
            }
            probability = mad_probability(tp.size, tp.titanium, band, false, false, localized)?;
            rule = "5.7.1";
            fix = Some("good".into());
            trace = json!({"localized":localized,"range_nm":range_nm});
        }
        Receiver::LaserRangefinder { range_nm, .. } => {
            let r = laser_range(o.kind == PlatformKind::Ship, range_nm, env.precipitation)?;
            let cued = w.sides[o.side.index()]
                .contacts
                .values()
                .any(|c| c.about == target && !c.stale);
            let (gap, _, _) = cloud_effect(
                state,
                w.seed,
                w.tick,
                observer,
                target,
                o.altitude_m,
                t.altitude_m,
                env.cloud.as_ref(),
            )?;
            return Ok(ScanDecision::new(
                cued && gap && range <= r,
                false,
                json!({"rule":"5.6.1","range_nm":r,"cued":cued}),
            ));
        }
    }
    let (roll, reused) = trial(state, w.seed, w.tick, &format!("{rule}:{key}"), 180);
    let detected = (roll as f64) <= probability;
    let tr = state.tracks.entry(key).or_default();
    tr.held = detected;
    if !detected {
        tr.first_success = None;
    }
    if detected {
        tr.last_success = Some(w.tick);
        if matches!(&receiver_info, Receiver::Sonar { .. }) && !bearing_only {
            tr.first_success = None;
        } else if tr.first_success.is_none() {
            tr.first_success = Some(w.tick);
        }
        if fix.as_deref() == Some("els_candidate") {
            if tr.last_els_window != Some(w.tick.div_euclid(180))
                && tr.last_els_position.is_none_or(|p| p.range_to(o.pos) > 0.0)
            {
                tr.last_els_window = Some(w.tick.div_euclid(180));
                tr.els_fixes += 1;
                tr.last_els_position = Some(o.pos)
            }
            fix = Some(if tr.els_fixes >= 2 { "good" } else { "fair" }.into());
        }
    }
    let mut d = ScanDecision::new(
        detected,
        bearing_only,
        json!({"rule":rule,"probability":probability,"d100":roll,"reused":reused,"inputs":trace}),
    );
    if matches!(&receiver_info,Receiver::Sonar{..}) {
        d.sonar_attempt=Some(super::communications2026::SonarAttempt{observer,target,at:w.tick});
        array_optional_world::after_search(w,state,observer,sensor,target,&d);
    }
    if detected {
        match receiver_info {
            Receiver::Visual { .. } => d.known_category = Some(t.combat_kind()),
            Receiver::Ir { .. } if class => d.known_category = Some(t.combat_kind()),
            Receiver::Mad { .. } => d.known_category = Some(PlatformKind::Sub),
            Receiver::Sonar { frequency, .. } => {
                let object = super::ew2026::object(w, target);
                if object.is_some_and(|o| o.kind == super::ew2026::ObjectKind::SonarJammer) {
                    return Ok(d);
                }
                if array_optional_world::classification_allowed(w,state,observer,sensor,target)&&bearing_only
                    && t.kind != PlatformKind::Aircraft
                    && track
                        .first_success
                        .is_some_and(|at| at.div_euclid(180) < w.tick.div_euclid(180))
                {
                    let (classification, reused) = trial(
                        state,
                        w.seed,
                        w.tick,
                        &format!("5.4.classify:{}:{sensor}:{}", observer.0, target.0),
                        180,
                    );
                    let level = sonar_classification(
                        classification,
                        probability,
                        frequency,
                        t.kind == PlatformKind::Torpedo,
                        false,
                        object
                            .filter(|o| o.kind == super::ew2026::ObjectKind::MobileDecoy)
                            .and_then(|o| o.generation.map(usize::from)),
                    )?;
                    if level >= 1 {
                        d.known_category = Some(t.combat_kind())
                    }
                    if level >= 3 {
                        class = true
                    }
                    if let Some(object) =
                        object.filter(|o| o.kind == super::ew2026::ObjectKind::MobileDecoy)
                    {
                        if level >= 4 {
                            d.class_label = Some("Mobile submarine decoy".into());
                        } else if level >= 3 {
                            d.class_label = object
                                .imitated_class
                                .as_ref()
                                .and_then(|id| w.db.platforms.get(id).map(|p| p.name.clone()));
                        }
                    }
                    d.trace["classification"] =
                        json!({"d100":classification,"reused":reused,"level":level});
                }
            }
            _ => {}
        }
    }
    d.identify_class = detected && class;
    d.bearing_error_deg = error;
    d.fix_quality = if detected { fix } else { None };
    Ok(d)
}

pub fn acoustic_intercept(
    generation: usize,
    frequency: Frequency,
    modified_active50: f64,
    range: f64,
    direct_path: bool,
) -> Result<(bool, Option<f64>, bool, bool), String> {
    if !(1..=5).contains(&generation) || frequency.idx() < 2 {
        return Err("5.4.5: AIR supports only specified generations and LMF/MF/HF".into());
    }
    valid_nonnegative(modified_active50)?;
    valid_nonnegative(range)?;
    let maximum = modified_active50
        * n(&tables()["sonar"]["air_multipliers"][generation - 1][frequency.idx() - 2]);
    Ok((
        range <= maximum,
        if direct_path {
            Some(n(&tables()["sonar"]["air_bearing_deg"][generation - 1]))
        } else {
            None
        },
        generation >= 3,
        generation == 5 && range <= 12.0 && direct_path,
    ))
}

/// v4 radar envelope extends v3 source bindings without mutating their archived
/// semantics. Formation signature and fleeting-mast signature are applied BEFORE
/// Annex lookup, never as an arbitrary multiplier of horizon-limited range.
pub fn radar_range_world(
    w: &World,
    state: &State,
    o: &crate::world::Entity,
    def: &crate::db::SensorDef,
    t: &crate::world::Entity,
) -> Result<f64, String> {
    if !super::expanded_damage2026::sensor_operational(w,o.id,&def.id)? {return Ok(0.);}
    if super::ew2026::object(w, t.id).is_some_and(|o| {
        !matches!(
            o.kind,
            super::ew2026::ObjectKind::LongRangeChaff | super::ew2026::ObjectKind::FloatingAsm
        )
    }) {
        return Ok(0.);
    }
    let p = super::aviation2026::operations_radar_profile(w, o, def)
        .ok_or("5.2: radar fit not source-verified")?;
    let env = w
        .context
        .radar_environment
        .as_ref()
        .filter(|e| e.valid())
        .ok_or("5.2.9: missing explicit radar environment")?;
    if w.weather.jam.iter().any(|j| *j > 0.0) {
        return Err("5.2.9: legacy jam scalar not source metadata".into());
    }
    if o.systems.radar <= 0.05 || !crate::sensors::radar_emitting(o) {
        return Ok(0.0);
    }
    let ops = super::operations::tables();
    let air = matches!(t.kind, PlatformKind::Aircraft | PlatformKind::Missile);
    let mast = t.kind == PlatformKind::Sub && t.depth_m > 0.0;
    let signature = if let Some(signature) = super::ew2026::radar_signature(w, t.id) {
        signature
    } else if mast {
        let m = state
            .masts
            .get(&t.id)
            .ok_or("5.2.1: no explicit raised mast state")?;
        if m.count == 0 || t.depth_m > 25.0 || t.speed_kts > 15.0 {
            return Ok(0.0);
        }
        if m.count >= 2 {
            3
        } else {
            4
        }
    } else if let Some(s) = state
        .config
        .targets
        .get(&t.class)
        .and_then(|t| t.radar_signature)
    {
        s
    } else if t.kind == PlatformKind::Aircraft {
        let a = super::aviation2026::operations_aircraft(w, &t.class)
            .ok_or("5.2.2: aircraft signature unverified")?;
        let mut sig = a["signature"]
            .as_u64()
            .ok_or("5.2.2: aircraft signature unverified")? as usize;
        if a["front_half_deg"]
            .as_f64()
            .is_some_and(|half| angle_diff(t.course_deg, t.pos.bearing_to(o.pos)).abs() <= half)
        {
            sig = a["front_signature"]
                .as_u64()
                .ok_or("front signature unverified")? as usize
        }
        let external = a["external_ordnance"] == true && t.weapons.iter().any(|m| m.ammo > 0);
        let count = state
            .groups
            .values()
            .find(|g| g.contains(&t.id))
            .map_or(1, |g| {
                g.iter()
                    .filter(|id| {
                        w.entity(**id)
                            .is_some_and(|e| e.alive && e.kind == PlatformKind::Aircraft)
                    })
                    .count()
            });
        group_signature(sig, external, count)?
    } else if t.kind == PlatformKind::Missile {
        let weapon = t
            .projectile
            .as_ref()
            .map(|p| p.weapon.as_str())
            .unwrap_or(&t.class);
        ops["radar"]["missile_signatures"][weapon]
            .as_u64()
            .or_else(||super::surface2026::antiship::incoming_radar_signature(w,weapon).map(|n|n as u64))
            .or_else(||super::aaw2026::incoming_radar_signature(w,weapon).map(|n|n as u64))
            .ok_or("5.2.2: missile signature unverified")? as usize
    } else {
        ops["radar"]["ship_signatures"][&t.class]
            .as_u64()
            .ok_or("5.2.2: target signature unverified")? as usize
    };
    let signature = super::signature_damage2026::project_radar(w, t.id, signature)?;
    if signature > 4 {
        return Err("5.2.2: signature index invalid".into());
    }
    if !air
        && !mast
        && t.kind != PlatformKind::Ship
        && t.combat_kind() != PlatformKind::Ship
        && !(t.kind == PlatformKind::Base
            && matches!(
                state
                    .config
                    .bindings
                    .get(&format!("{}:{}", o.class, def.id)),
                Some(Receiver::RadarFeatures { sar: true, .. })
            ))
    {
        return Ok(0.0);
    }
    if angle_diff(o.course_deg, o.pos.bearing_to(t.pos)).abs() > n(&p["arc"]) / 2.0 {
        return Ok(0.0);
    }
    let role = p["role"].as_str().ok_or("5.2: radar role missing")?;
    let altitude = if air {
        super::operations::altitude_modifier(
            role,
            o.kind == PlatformKind::Aircraft,
            o.altitude_m,
            t.altitude_m,
        )
    } else {
        1.0
    };
    let low = !air || t.altitude_m <= 60.0;
    if w.context
        .terrain
        .as_ref()
        .is_some_and(|c| c.blocked(o.pos, t.pos))
        && o.kind == PlatformKind::Ship
        && !air
    {
        return Ok(0.0);
    }
    if w.context.terrain.is_some() && w.context.radar_land_clutter.is_none() {
        return Err("5.2.9.3: unclassified land clutter".into());
    }
    let base = n(&p[if air { "air" } else { "surface" }][signature]);
    let land = w.context.radar_land_clutter.as_ref().and_then(|rs| {
        rs.iter()
            .filter(|r| r.affects(o.pos, o.pos.bearing_to(t.pos), base))
            .map(|r| r.terrain)
            .max()
    });
    let generation = p["generation"]
        .as_u64()
        .ok_or("5.2: radar generation missing")? as usize;
    let clutter = super::operations::clutter_modifier(
        generation,
        o.kind != PlatformKind::Aircraft && !air,
        o.kind != PlatformKind::Aircraft && air && matches!(role, "AS" | "3D") && low,
        env.sea_state,
        &env.precipitation,
        land,
        low,
    )?;
    let ew = super::electronic_warfare::range_modifier_for(w, o, def, t, generation);
    let observer_height=super::operations::radar_height(o)?;
    let target_height=if let Some(object) = super::ew2026::object(w, t.id) {
            object
                .radar_height_m
                .ok_or("chapter12: source reflector/cloud height absent")?
        } else if mast {
            0.5
        } else {
            if t.kind == PlatformKind::Base {
                *state.config.target_heights_m.get(&t.class).ok_or("5.2.3 SAR: target elevation/height requires source or explicit scenario geometry")?
            } else {
                super::operations::radar_height(t)?
            }
        };
    let h = crate::geom::horizon_nm(observer_height,target_height);
    let h = if low && matches!(role, "SS" | "Nav" | "SS-T" | "LAS") {
        if let Some(duct) = &state.config.duct {
            if duct.source.trim().is_empty() {
                return Err("5.2.9.5: duct provenance absent".into());
            }
            let own = duct_size(w, o)?;
            if duct.surface || own >= duct.max_observer_size {
                let target = if air {
                    if t.altitude_m <= 20.0 {
                        4
                    } else {
                        return Err("5.2.9.5: VLow contact antenna height not represented in surface duct table".into());
                    }
                } else {
                    duct_size(w, t)?
                };
                duct_horizon(duct.surface, own, target)?
            } else {
                h
            }
        } else {
            h
        }
    } else {
        h
    };
    let range=(base * altitude * clutter.min(ew)).min(h);
    if range>0. && o.pos.range_to(t.pos)<=range
        && super::ew2026::airborne_chaff::radar_los(w,o.id,t.id,observer_height,target_height)? {
        return Ok(0.);
    }
    Ok(range)
}

fn mast_contact(t: &crate::world::Entity) -> bool {
    t.kind == PlatformKind::Sub && t.depth_m > 0.0
}
/// Contact truth categories remain internal. v4 public observations use only
/// the classifications actually obtained by a sensor on this side.
pub fn known_category(w: &World, side: crate::Side, about: EntityId) -> Option<PlatformKind> {
    if w.rules.profile != super::damage_runtime::RuleProfile::Rts2026Full {
        return w.entity(about).map(|e| e.combat_kind());
    }
    w.rules
        .detection
        .as_ref()?
        .categories
        .get(&format!("{}:{}", side.index(), about.0))
        .copied()
}
pub fn active(w: &World) -> bool {
    w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full
}

/// §5.4.6 target strength and explicit Aspect Diagram10-2/PDF111.
pub fn active_aspect_modifier(relative: f64) -> i32 {
    let a = angle_diff(relative, 0.0).abs();
    if a <= 20.0 || a >= 160.0 {
        -1
    } else if (60.0..=120.0).contains(&a) {
        1
    } else {
        0
    }
}

impl Environment {
    pub fn validate(&self) -> Result<(), String> {
        if self.source.trim().is_empty()
            || self.sea_state > 9
            || self.precipitation > 5
            || self.shipping > 3
            || self.biologics > 3
            || self.ice > 2
            || self.layer.is_some_and(|l| l > 2)
        {
            return Err("chapter5: incomplete/invalid explicit environment".into());
        }
        valid_nonnegative(self.water_depth_m)?;
        visibility_row(self.visibility_percent)?;
        if self.layer.is_some() && (self.sea_state >= 6 || self.ice > 0) {
            return Err("5.4.7.1: layer cannot form inSS6+ or ice".into());
        }
        if self.layer == Some(2) && self.water_depth_m <= 200.0 {
            return Err("5.4.7.1: Deep layer cannot form in shallow water".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "path", rename_all = "snake_case")]
pub enum Propagation {
    Direct,
    BottomBounce { rocky_flat: bool },
    ConvergenceZone { polar: bool, mediterranean: bool },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Duct {
    pub source: String,
    pub surface: bool,
    pub max_observer_size: usize,
}
fn duct_size(_w: &World, e: &crate::world::Entity) -> Result<usize, String> {
    if e.kind == PlatformKind::Sub && e.depth_m > 0.0 {
        return Ok(4);
    }
    if e.kind == PlatformKind::Aircraft {
        return Err("5.2.9.5: aircraft antenna duct-height metadata missing".into());
    }
    let size = super::operations::tables()["radar"]["physical_sizes"][&e.class]
        .as_str()
        .ok_or("5.2.9.5: duct observer physical size not source verified")?;
    Ok(match size {
        "A" => 0,
        "B" => 1,
        "C" | "D" => 2,
        "E" | "F" | "G" => 3,
        _ => return Err("5.2.9.5: unsupported duct size".into()),
    })
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StarshellEffect {
    pub works: bool,
    pub illuminated: bool,
    pub silhouetted: bool,
    pub blinds_firer: bool,
    pub offset_kyd: Vec2,
    pub radius_yards: f64,
}
/// §5.8.8 starshell range/illumination and PDF68 miss diagram. Coordinates:
/// x lateral, y long (positive beyond target). Each printed square is2kyd.
pub fn starshell_effect(
    caliber_mm: f64,
    range_kyd: f64,
    hit: bool,
    miss_d100: u8,
) -> Result<StarshellEffect, String> {
    valid_nonnegative(caliber_mm)?;
    valid_nonnegative(range_kyd)?;
    let row = tables()["visual"]["starshell_guns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| caliber_mm >= n(&r[0]) && caliber_mm <= n(&r[1]))
        .ok_or("5.8.8: caliber outside verified starshell table")?;
    let radius = n(&row[3]);
    let offset = if hit {
        Vec2::zero()
    } else {
        if !(1..=100).contains(&miss_d100) {
            return Err("5.8.8: invalid starshell miss d100".into());
        }
        let index = ((miss_d100 - 1) / 5) as usize;
        // Printed20 box ranges31–35,36–40,16–20,41–45,46–50 /21–25,...
        let cell = &tables()["visual"]["starshell_miss_cells"][index];
        Vec2::new(n(&cell[0]), n(&cell[1]))
    };
    let works =
        range_kyd >= n(&row[2]) && (hit || miss_d100 < 96) && range_kyd + offset.y >= n(&row[2]);
    let illuminated = works && hit;
    // Gray boxes:01–15 (up to2kyd long), plus16–20 (center4kyd long)
    // diagram gray also16–20 but operative text says up to2kyd; block that conflict.
    if !hit && (16..=20).contains(&miss_d100) {
        return Err("5.8.8: gray16–20 diagram is4kyd long but prose limits silhouette to2kyd; pending adjudication".into());
    }
    Ok(StarshellEffect {
        works,
        illuminated,
        silhouetted: works && !hit && miss_d100 <= 15,
        blinds_firer: works && offset.y < 0.0,
        offset_kyd: offset,
        radius_yards: radius,
    })
}
pub fn starshell_detection_range(size: usize, silhouette: bool) -> Result<f64, String> {
    if size > 6 {
        return Err("5.8.8: target size missing".into());
    }
    Ok(n(
        &tables()["visual"]["starshell_ranges"][size][usize::from(silhouette)]
    ))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArrayJob {
    pub recovering: bool,
    pub ready_at: i64,
    pub deployed: bool,
    pub lost: bool,
    pub depth_m: f64,
}
/// Runtime deployment, no invented sonar depth. 21min streaming/15kt limit;
/// clip-on arrays must be declared as initially deployed and cannot be recovered.
pub fn order_array(
    w: &mut World,
    unit: EntityId,
    sensor: &str,
    deploy: bool,
    depth_m: f64,
) -> Result<(), String> {
    valid_nonnegative(depth_m)?;
    let o = w.entity(unit).ok_or("5.4.2: missing unit")?;
    if o.speed_kts > 15.0 {
        return Err("5.4.2: streaming/retrieving requires speed<=15kt".into());
    }
    let class = o.class.clone();
    let state = w
        .rules
        .detection
        .as_mut()
        .ok_or("5.4.2: v4 state required")?;
    let key = format!("{}:{sensor}", unit.0);
    let Some(Receiver::Sonar {
        mount: SonarMount::Towed | SonarMount::Vds,
        ..
    }) = state.config.bindings.get(&format!("{}:{sensor}", class))
    else {
        return Err("5.4.2: fitted towed/VDS capability required".into());
    };
    if state.arrays.get(&key).is_some_and(|j| j.lost) {
        return Err("5.4.2: array already lost".into());
    }
    if !deploy && !state.arrays.get(&key).is_some_and(|j| j.deployed) {
        return Err("5.4.2: no streamed array to retrieve".into());
    }
    let duration = tables()["sonar"]["stream_s"].as_i64().unwrap();
    state.arrays.insert(
        key,
        ArrayJob {
            recovering: !deploy,
            ready_at: w.tick + duration,
            deployed: false,
            lost: false,
            depth_m,
        },
    );
    Ok(())
}
pub fn array_speed_limit(w: &World, unit: EntityId) -> Option<f64> {
    let s = w.rules.detection.as_ref()?;
    let o = w.entity(unit)?;
    let mut limit: f64 = if s.ultra_quiet.contains(&unit) {
        3.0
    } else {
        f64::INFINITY
    };
    for (key, job) in &s.arrays {
        if !key.starts_with(&format!("{}:", unit.0)) || job.lost {
            continue;
        }
        if !job.deployed && w.tick < job.ready_at {
            limit = limit.min(15.0)
        }
        if job.deployed {
            let sensor = key.split_once(':')?.1;
            if matches!(
                s.config.bindings.get(&format!("{}:{sensor}", o.class)),
                Some(Receiver::Sonar {
                    mount: SonarMount::Towed | SonarMount::Vds,
                    active_range_nm: Some(_),
                    ..
                })
            ) {
                limit = limit.min(25.0)
            }
        }
    }
    if limit.is_finite() {
        Some(limit)
    } else {
        None
    }
}
/// Called immediately after the production release event, while its exact
/// projectile is still available. Never infer a stable ID from a name fragment
/// or projectile age; the association itself survives full-World persistence.
pub fn register_projectile_release(w: &mut World, projectile: EntityId) {
    if !active(w) {
        return;
    }
    let Some(p) = w.entity(projectile).and_then(|e| e.projectile.as_ref()) else {
        return;
    };
    let Some(def) = w.db.weapons.get(&p.weapon) else {
        return;
    };
    let Some(event) = w.events.back() else {
        return;
    };
    let crate::EventKind::WeaponsRelease {
        shooter,
        side,
        weapon,
        ..
    } = &event.kind
    else {
        return;
    };
    if *shooter != p.launched_by || *side != p.launched_by_side || weapon != &def.name {
        return;
    }
    let release = ReleaseWeapon {
        projectile,
        weapon: p.weapon.clone(),
    };
    let seq = event.seq;
    if let Some(state) = w.rules.detection.as_mut() {
        state.release_weapons.insert(seq, release);
    }
}

fn release_weapon<'a>(
    w: &'a World,
    state: &State,
    seq: u64,
    label: &str,
) -> Result<&'a crate::db::WeaponDef, String> {
    if let Some(release) = state.release_weapons.get(&seq) {
        return w
            .db
            .weapons
            .get(&release.weapon)
            .filter(|d| d.name == label || d.id == label)
            .ok_or("stable projectile weapon is absent or conflicts with release event".into());
    }
    // Compatibility for earlier v4 fixtures/events that explicitly used IDs.
    if let Some(def) = w.db.weapons.get(label) {
        return Ok(def);
    }
    let mut exact = w.db.weapons.values().filter(|d| d.name == label);
    match (exact.next(), exact.next()) {
        (Some(def), None) => Ok(def),
        (Some(_), Some(_)) => Err(
            "display name identifies multiple weapon IDs; release identity requires adjudication"
                .into(),
        ),
        _ => Err("release weapon identity is not source-verified".into()),
    }
}

/// Called by actual source scan pass. All exceptional losses and completion
/// state persist, preventing reload rerolls. Returns referee-only evidence.
pub fn update_lifecycle(w: &World, state: &mut State) -> Vec<(EntityId, Value)> {
    let floor = w.tick.div_euclid(180) - 4;
    state.trials.retain(|_, t| t.window >= floor);
    state.reported.retain(|_, (window, _)| *window >= floor);
    let mut traces = Vec::new();
    for event in w.events.iter().filter(|e| e.seq > state.last_event_seq) {
        if let crate::EventKind::WeaponsRelease {
            shooter, weapon, ..
        } = &event.kind
        {
            state.fired.insert(*shooter);
            if let Some(e) = w.entity(*shooter).filter(|e| e.kind == PlatformKind::Sub) {
                let duration = tables()["sonar"]["launch_transients"]["duration_s"]
                    .as_i64()
                    .unwrap();
                match release_weapon(w, state, event.seq, weapon) {
                    Ok(def) => {
                        let source = &tables()["sonar"]["launch_transients"];
                        let floor = match def.kind {
                            crate::db::WeaponKind::Torpedo | crate::db::WeaponKind::Lwt => {
                                Some(if state.config.quiet_launches.contains(&e.class) {
                                    i(&tables()["sonar"]["noise_modifiers"][source
                                        ["quiet_torpedo_noise_index"]
                                        .as_u64()
                                        .unwrap()
                                        as usize])
                                } else {
                                    i(&tables()["sonar"]["noise_modifiers"]
                                        [source["torpedo_noise_index"].as_u64().unwrap() as usize])
                                })
                            }
                            crate::db::WeaponKind::Ascm
                            | crate::db::WeaponKind::Slcm
                            | crate::db::WeaponKind::Sam
                            | crate::db::WeaponKind::Aam
                            | crate::db::WeaponKind::AswRocket => Some(i(&tables()["sonar"]
                                ["noise_modifiers"]
                                [source["missile_noise_index"].as_u64().unwrap() as usize])),
                            _ => None,
                        };
                        if let Some(floor) = floor {
                            state
                                .transients
                                .entry(*shooter)
                                .and_modify(|(until, n)| {
                                    *until = (*until).max(event.tick + duration);
                                    *n = (*n).max(floor)
                                })
                                .or_insert((event.tick + duration, floor));
                            traces.push((*shooter,json!({"rule":"5.4.6.5.launch_transient","weapon":weapon,"weapon_id":def.id,"projectile":state.release_weapons.get(&event.seq).map(|r|r.projectile),"noise_floor":floor,"until":event.tick+duration})));
                        }
                    }
                    Err(reason) => {
                        state
                            .unresolved_transients
                            .entry(*shooter)
                            .and_modify(|(until, old)| {
                                *until = (*until).max(event.tick + duration);
                                *old = reason.clone();
                            })
                            .or_insert((event.tick + duration, reason.clone()));
                        traces.push((*shooter,json!({"rule":"5.4.6.5.launch_transient_pending","weapon":weapon,"reason":reason,"until":event.tick+duration})));
                    }
                }
            }
        }
    }
    state.last_event_seq = w.event_seq;
    state
        .release_weapons
        .retain(|seq, _| *seq > state.last_event_seq);
    state.transients.retain(|_, (until, _)| *until > w.tick);
    state
        .unresolved_transients
        .retain(|_, (until, _)| *until > w.tick);
    let keys: Vec<_> = state.arrays.keys().cloned().collect();
    for key in keys {
        let Some((unit, sensor)) = key
            .split_once(':')
            .and_then(|(u, s)| u.parse::<u32>().ok().map(|u| (EntityId(u), s.to_string())))
        else {
            continue;
        };
        let Some(o) = w.entity(unit).filter(|o| o.alive) else {
            continue;
        };
        let job = state.arrays[&key].clone();
        if job.lost {
            continue;
        }
        if matches!(
            state.config.bindings.get(&format!("{}:{sensor}", o.class)),
            Some(Receiver::Sonar {
                mount: SonarMount::Dipping,
                ..
            })
        ) {
            if job.deployed
                && (o.speed_kts > n(&tables()["sonar"]["dipping"]["max_horizontal_speed_kts"])
                    || state
                        .array_positions
                        .get(&key)
                        .is_some_and(|p| p.range_to(o.pos) > 0.0))
            {
                state.arrays.get_mut(&key).unwrap().lost = true;
                traces.push((unit,json!({"rule":"5.4.3.cable_loss","sensor":sensor,"reason":"horizontal_motion","speed_kts":o.speed_kts})));
            }
            // Dipping sonars are lowered immediately and can be recovered in
            // the same turn; towed-array sea-loss and streaming rules differ.
            continue;
        }
        if !job.deployed && w.tick < job.ready_at {
            if o.speed_kts > 15.0 {
                state.arrays.get_mut(&key).unwrap().ready_at =
                    w.tick + tables()["sonar"]["stream_s"].as_i64().unwrap();
                traces.push((unit,json!({"rule":"5.4.2.RTS_stream_restart","reason":"unsafe_speed","sensor":sensor})));
            }
            continue;
        }
        if !job.deployed && w.tick >= job.ready_at {
            state.arrays.get_mut(&key).unwrap().deployed = !job.recovering;
            if job.recovering {
                state.arrays.remove(&key);
                continue;
            } else {
                traces.push((
                    unit,
                    json!({"rule":"5.4.2.deployed","sensor":sensor,"depth_m":job.depth_m}),
                ));
            }
        }
        let Some(env) = state.config.environment.as_ref() else {
            continue;
        };
        let water = env.water_depth_m;
        let sea = env.sea_state;
        if water < job.depth_m {
            state.arrays.get_mut(&key).unwrap().lost = true;
            traces.push((unit,json!({"rule":"5.4.2.seabed_loss","sensor":sensor,"depth_m":job.depth_m,"water_depth_m":water})));
            continue;
        }
        if sea > 5 && o.kind == PlatformKind::Ship {
            let (roll, reused) = trial(state, w.seed, w.tick, &format!("5.4.tow_loss:{key}"), 180);
            if !reused {
                if roll <= 50 {
                    state.arrays.get_mut(&key).unwrap().lost = true
                };
                traces.push((unit,json!({"rule":"5.4.2.sea_loss","d100":roll,"probability":50,"lost":roll<=50,"sensor":sensor})));
            }
        }
    }
    traces
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DetectionAction {
    FireStarshell { contact:crate::ContactId, weapon:String },
    EchoRange { initial_buoy: EntityId },
    DropSonobuoy { model:String, depth_m:f64, radio_channel:u16 },
    ListenSonobuoys { buoys:Vec<EntityId>, #[serde(default)] purpose:sonobuoys::Purpose },
    Laser { action:laser::Action },
    Soks { enabled:bool },
    WideMad { field:String, enabled:bool },
    UltraQuiet {
        enabled: bool,
    },
    Masts {
        count: u8,
    },
    Tow {
        sensor: String,
        deploy: bool,
        depth_m: f64,
    },
    Dip {
        sensor: String,
        deploy: bool,
        depth_m: f64,
    },
}
pub fn apply(w: &mut World, unit: EntityId, action: &DetectionAction) -> Result<(), String> {
    if !active(w) {
        return Err("detection actions require explicit hv2026-rts-v4".into());
    }
    match action {
        DetectionAction::FireStarshell{contact,weapon}=>{let side=w.entity(unit).ok_or("5.8.8 actor absent")?.side; super::surface2026::gunnery::prepare_starshell(w,side,unit,*contact,weapon)},
        DetectionAction::EchoRange{initial_buoy}=>sonobuoys::control(w,unit,&sonobuoys::Action::EchoRange{initial_buoy:*initial_buoy}),
        DetectionAction::DropSonobuoy{model,depth_m,radio_channel}=>sonobuoys::control(w,unit,&sonobuoys::Action::DropSonobuoy{model:model.clone(),depth_m:*depth_m,radio_channel:*radio_channel}),
        DetectionAction::ListenSonobuoys{buoys,purpose}=>sonobuoys::control(w,unit,&sonobuoys::Action::ListenSonobuoys{buoys:buoys.clone(),purpose:*purpose}),
        DetectionAction::Laser{action}=>laser::control(w,unit,action),
        DetectionAction::Soks{enabled}=>soks::control(w,unit,*enabled),
        DetectionAction::WideMad{field,enabled}=>wide_mad::control(w,unit,field,*enabled),
        DetectionAction::UltraQuiet { enabled } => {
            let e = w.entity(unit).ok_or("5.4.6.5: missing unit")?;
            if e.kind != PlatformKind::Sub {
                return Err("5.4.6.5: ultraquiet only applies to submarines".into());
            }
            let class = e.class.clone();
            let speed = e.speed_kts;
            let state = w.rules.detection.as_mut().ok_or("v4 required")?;
            if *enabled {
                if !matches!(
                    state.config.propulsion.get(&class).map(|s| s.as_str()),
                    Some("nuclear" | "conventional")
                ) {
                    return Err("5.4.6.5: verified nuclear/conventional propulsion required".into());
                }
                if speed > 3.0 {
                    return Err("5.4.6.5: ultra quiet max3kt".into());
                }
                state.ultra_quiet.insert(unit);
            } else {
                state.ultra_quiet.remove(&unit);
            }
            Ok(())
        }
        DetectionAction::Masts { count } => {
            let e = w.entity(unit).ok_or("5.2.1: missing unit")?;
            if e.kind != PlatformKind::Sub
                || (*count > 0 && (e.depth_m > 25.0 || e.speed_kts > 15.0))
            {
                return Err("5.2.1: mast requires periscope depth and<=15kt".into());
            }
            let class = e.class.clone();
            let state = w.rules.detection.as_mut().ok_or("v4 required")?;
            let max = state
                .config
                .max_masts
                .get(&class)
                .copied()
                .ok_or("5.2.1: raised mast fit/count not source verified")?;
            if *count > max {
                return Err("5.2.1: mast count exceeds source fit".into());
            }
            let since = state
                .masts
                .get(&unit)
                .filter(|m| m.count > 0 && *count > 0)
                .map_or(w.tick, |m| m.raised_since);
            state.masts.insert(
                unit,
                Mast {
                    count: *count,
                    raised_since: since,
                },
            );
            Ok(())
        }
        DetectionAction::Tow {
            sensor,
            deploy,
            depth_m,
        } => order_array(w, unit, sensor, *deploy, *depth_m),
        DetectionAction::Dip {
            sensor,
            deploy,
            depth_m,
        } => order_dipping(w, unit, sensor, *deploy, *depth_m),
    }
}

/// §5.4.3: no invented 21-minute tow delay for a dipping sonar. Once the
/// fitted platform is stationary it is available in the same detection turn.
pub fn order_dipping(
    w: &mut World,
    unit: EntityId,
    sensor: &str,
    deploy: bool,
    depth_m: f64,
) -> Result<(), String> {
    if !active(w) {
        return Err("5.4.3: dipping task requires v4".into());
    }
    valid_nonnegative(depth_m)?;
    let o = w.entity(unit).ok_or("5.4.3: missing unit")?;
    if !o.alive
        || !o.sensors.iter().any(|s| s.sensor == sensor)
        || crate::sensors::sensor_damaged(o, sensor)
    {
        return Err("5.4.3: operational fitted dipping sonar required".into());
    }
    let state = w
        .rules
        .detection
        .as_ref()
        .ok_or("5.4.3: v4 state missing")?;
    if !matches!(state.config.bindings.get(&format!("{}:{sensor}", o.class)), Some(Receiver::Sonar { mount: SonarMount::Dipping, source, .. }) if !source.trim().is_empty())
    {
        return Err("5.4.3: source verified dipping sonar fit required".into());
    }
    let key = format!("{}:{sensor}", unit.0);
    if state.arrays.get(&key).is_some_and(|j| j.lost) {
        return Err("5.4.3: dipping sonar cable already lost".into());
    }
    if deploy {
        if o.speed_kts > n(&tables()["sonar"]["dipping"]["max_horizontal_speed_kts"]) {
            return Err("5.4.3: dipping sonar requires stationary platform".into());
        }
        let eligible = match o.kind {
            PlatformKind::Ship => true,
            PlatformKind::Aircraft => match state.config.dipping_platforms.get(&o.class) {
                Some(DippingPlatform::Helicopter) => o.altitude_m > 0.0,
                Some(DippingPlatform::FlyingBoat) => o.altitude_m == 0.0,
                None => false,
            },
            _ => false,
        };
        if !eligible {
            return Err(
                "5.4.3: explicit hovering helicopter or landed flying boat required".into(),
            );
        }
        let env = state
            .config
            .environment
            .as_ref()
            .ok_or("5.4.3: explicit water depth required")?;
        env.validate()?;
        if depth_m > env.water_depth_m {
            return Err("5.4.3: dipping depth exceeds water depth".into());
        }
        let pos = o.pos;
        let state = w.rules.detection.as_mut().unwrap();
        state.array_positions.insert(key.clone(), pos);
        state.arrays.insert(
            key,
            ArrayJob {
                recovering: false,
                ready_at: w.tick
                    + tables()["sonar"]["dipping"]["deployment_delay_s"]
                        .as_i64()
                        .unwrap(),
                deployed: true,
                lost: false,
                depth_m,
            },
        );
    } else {
        let state = w.rules.detection.as_mut().unwrap();
        state.arrays.remove(&key);
        state.array_positions.remove(&key);
    }
    Ok(())
}

/// Own-side task/deployment view. Failed scans and enemy identities are absent.
pub fn own_view(w: &World, side: crate::Side, unit: EntityId) -> Option<Value> {
    if !active(w) || w.entity(unit)?.side != side {
        return None;
    }
    let s = w.rules.detection.as_ref()?;
    let prefix = format!("{}:", unit.0);
    let arrays:Vec<_>=s.arrays.iter().filter(|(k,_)|k.starts_with(&prefix)).map(|(k,j)|json!({"sensor":k.split_once(':').map(|(_,id)|id),"deployed":j.deployed,"recovering":j.recovering,"ready_at":j.ready_at,"lost":j.lost,"depth_m":j.depth_m})).collect();
    let mut view=json!({"arrays":arrays,"speed_limit_kts":array_speed_limit(w,unit),"masts":s.masts.get(&unit),"ultra_quiet":s.ultra_quiet.contains(&unit),"electronic_warfare":super::ew2026::own_view(w,side,unit),"wide_mad":wide_mad::own_view(w,unit),"soks":soks::own_view(w,side,unit),"laser":laser::own_view(w,side,unit),"sonobuoys":sonobuoys::own_view(w,side,unit),"sensor_float":sonobuoys::is_sensor_float(w,unit),"visual_identification":visual_id::own_view(w,side,unit),"visual_signaling":visual_signaling::own_view(w,side,unit)});
    let lights=super::surface2026::gunnery::visible_lights(w,unit);
    if !lights.is_empty(){view["visible_lights"]=json!(lights);}
    Some(view)
}
/// Passive signal strength for pre-roll masking. It consumes the SAME source
/// tables and explicit path choice as a detection trial; no information is sent
/// to a player merely because the referee compares two eligible sound sources.
fn passive_strength(
    w: &World,
    state: &State,
    o: &crate::world::Entity,
    t: &crate::world::Entity,
    sensor: &str,
    receiver: &Receiver,
) -> Result<f64, String> {
    let Receiver::Sonar {
        generation,
        passive_range_nm: Some(base),
        frequency,
        mount,
        high_speed_tow,
        ..
    } = receiver
    else {
        return Err("passive source fit required".into());
    };
    if !sonar_bearing_eligible(
        *mount,
        *generation,
        angle_diff(o.course_deg, o.pos.bearing_to(t.pos)),
    ) {
        return Ok(0.0);
    }
    let tp = sensed_profile(w, &state.config, t, true)?;
    let noise = tp.noise.ok_or("passive source noise missing")?;
    if noise > 4 {
        return Err("invalid noise".into());
    }
    let Some(relative) = sonar_speed_modifier(false, t.speed_kts, o.speed_kts, *high_speed_tow)?
    else {
        return Ok(0.0);
    };
    let mut env = state
        .config
        .environment
        .as_ref()
        .ok_or("explicit acoustic environment required")?.clone();
    let nuclear_blueout=super::nuclear2026::blueout_active(w,o.id,sensor);
    if nuclear_blueout {env.sea_state=super::nuclear2026::tables()["rules"]["blueout_sea_state"].as_u64().unwrap() as usize;}
    let raw_mod = passive_signature(w,t,noise,tp.pumpjet,false)?;
    let target_mod = passive_source_modifier(w, state, t, *frequency, raw_mod)?;
    let environment = sonar_environment(false, *frequency, &env)?;
    let sd = deployed_sonar_depth(state, o, sensor, *mount)?;
    let alert = ordinary_sonar_alerted(w,state,o.id,t.id)||super::communications2026::sonar_alert(w,o.id,t.id);
    let range = o.pos.range_to(t.pos);
    match state
        .config
        .propagation
        .get(&format!("{}:{sensor}", o.class))
        .cloned()
        .unwrap_or(Propagation::Direct)
    {
        Propagation::Direct => sonar_probability(
            base * sonar_range_modifier(
                target_mod
                    + relative
                    + environment
                    + layer_modifier(*frequency, true, sd, t.depth_m, env.layer)?,
            )?,
            range,
            alert,
        ),
        Propagation::BottomBounce { .. } => Err("passive bottom bounce is not supported".into()),
        Propagation::ConvergenceZone {
            polar,
            mediterranean,
        } => {
            if nuclear_blueout {return Ok(0.);}
            let count = cz_count(
                false,
                *generation,
                *frequency,
                if super::navigation2026::submarine::emergency_blowing(w, t.id) {
                    0
                } else {
                    super::signature_damage2026::project_noise(w, t.id, noise)?
                },
                t.speed_kts,
                o.speed_kts,
                mediterranean,
            )?;
            let zone = (1..=count).find(|z| {
                cz_interval(*z, polar, mediterranean, env.water_depth_m)
                    .ok()
                    .flatten()
                    .is_some_and(|(a, b)| range >= a && range <= b)
            });
            if let Some(z) = zone {
                propagation_probability(
                    target_mod + relative + environment,
                    *generation,
                    Some(z),
                    alert,
                )
            } else {
                Ok(0.0)
            }
        }
    }
}

/// One physical deployment state drives own scanning, acoustic interception,
/// masking and layer crossing. A stale ext depth cannot resurrect a recovered
/// or broken array.
fn deployed_sonar_depth(
    state: &State,
    unit: &crate::world::Entity,
    sensor: &str,
    mount: SonarMount,
) -> Result<f64, String> {
    let depth = match mount {
        SonarMount::Towed | SonarMount::Vds | SonarMount::Dipping => state
            .arrays
            .get(&format!("{}:{sensor}", unit.id.0))
            .filter(|a| a.deployed && !a.lost)
            .map(|a| a.depth_m)
            .ok_or("5.4.2/5.4.3: deployed working array required")?,
        SonarMount::Buoy => unit
            .ext
            .get(&format!("detection.sonar_depth.{sensor}"))
            .copied()
            .ok_or("5.4.4: explicit buoy depth required")?,
        _ => unit.depth_m,
    };
    valid_nonnegative(depth)?;
    Ok(depth)
}

fn passive_source_modifier(
    w: &World,
    state: &State,
    target: &crate::world::Entity,
    frequency: Frequency,
    base: i32,
) -> Result<i32, String> {
    if let Some((_, reason)) = state
        .unresolved_transients
        .get(&target.id)
        .filter(|(until, _)| *until > w.tick)
    {
        return Err(format!("5.4.6.5: release transient pending: {reason}"));
    }
    let mut signature = base;
    // The public command is an actual committed LMF active emission. Ordinary
    // passive listening uses this receiver's own base range and noise +8;
    // the command's 5nm active base is reserved for the separate AIR path.
    if super::unmanned2026::acoustic_command_sources(w).iter().any(|s|s.emitter==target.id && hears_active(frequency,s.frequency)) {
        signature=signature.max(i(&tables()["sonar"]["active_emission_noise_modifier"]));
    }
    if state.ultra_quiet.contains(&target.id) && target.speed_kts <= 3.0 {
        signature += match state
            .config
            .propulsion
            .get(&target.class)
            .map(|s| s.as_str())
        {
            Some("nuclear") => -1,
            Some("conventional") => -2,
            _ => return Err("5.4.6.5: ultraquiet propulsion unknown".into()),
        };
    }
    if let Some((until, floor)) = state
        .transients
        .get(&target.id)
        .filter(|(until, _)| *until > w.tick)
    {
        let _ = until;
        signature = signature.max(*floor);
    }
    for sensor in &target.sensors {
        if sensor.active
            && w.db
                .sensors
                .get(&sensor.sensor)
                .is_some_and(|d| d.kind == crate::db::SensorKind::SonarActive)
            && !crate::sensors::sensor_damaged(target, &sensor.sensor)
        {
            if let Some(Receiver::Sonar {
                frequency: active_frequency,
                active_range_nm: Some(_),
                mount,
                ..
            }) = state
                .config
                .bindings
                .get(&format!("{}:{}", target.class, sensor.sensor))
            {
                if hears_active(frequency, *active_frequency)
                    && deployed_sonar_depth(state, target, &sensor.sensor, *mount).is_ok()
                    && target.systems.sonar > 0.05
                    && !(crate::rules::damage_runtime::marker(target)
                        && target.ext.get("damage.power_lost").copied().unwrap_or(0.0) > 0.0)
                {
                    signature = signature.max(8)
                }
            }
        }
    }
    Ok(signature)
}

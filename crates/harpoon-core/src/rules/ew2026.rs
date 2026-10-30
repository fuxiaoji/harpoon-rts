//! Chapter 12 source-backed deployed EW objects, explicitly opt-in v4.
//! Objects live in referee runtime rather than the combat entity collection:
//! a false echo must never count as a killed ship or acquire a ship's weapons.
//! The detection pass materializes sensor-only targets and removes them before
//! returning. Unknown fits, endurance and low-speed energy curves are blocked.
use crate::types::PlatformKind;
use crate::world::Entity;
use crate::{EntityId, Side, Vec2, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;
mod mobile_speed;
pub mod airborne_chaff;
pub mod floating_tow;
pub(crate) fn validate_mobile_speed(w:&World,unit:EntityId,object:EntityId,speed_kts:f64)->Result<(),String>{
    mobile_speed::validate(w,unit,object,speed_kts)
}
pub(crate) fn validate_action(w:&World,unit:EntityId,action:&Action)->Result<(),String>{
    match action {
        Action::MobileSpeed{object,speed_kts}=>validate_mobile_speed(w,unit,*object,*speed_kts),
        Action::FloatingTow{object,geometry}=>floating_tow::validate_attach(w,unit,*object,geometry),
        Action::FloatingUntow{object}=>floating_tow::validate_detach(w,unit,*object),
        Action::StartChaffBarrier{fit}=>airborne_chaff::validate_start_stop(w,unit,Some(fit)),
        Action::StopChaffBarrier=>airborne_chaff::validate_start_stop(w,unit,None),
        _=>Ok(()),
    }
}
pub fn initialize(w:&mut World)->Result<(),String>{
    if !active(w){return Ok(());}
    let config=w.rules.ew.as_ref().and_then(|s|s.config.airborne_chaff.clone());
    let state=airborne_chaff::initialize(w,config.as_ref())?;
    if let Some(ew)=w.rules.ew.as_mut(){ew.airborne_chaff=state;}
    Ok(())
}

pub fn tables() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/ew-2026.json"))
            .expect("verified chapter12 source tables")
    })
}
fn number(section: &str, field: &str) -> f64 {
    tables()[section][field]
        .as_f64()
        .expect("verified source number")
}
fn seconds(section: &str, field: &str) -> i64 {
    tables()[section][field]
        .as_i64()
        .expect("verified source seconds")
}
pub fn active(w: &World) -> bool {
    super::detection2026::active(w)
}
pub(crate) fn mobile_mad_capable(generation:Option<u8>)->Result<bool,String>{
    mobile_row(generation.ok_or("12.6.2 source mobile simulator generation absent")?)?["mad"]
        .as_bool().ok_or_else(||"12.6.2 source MAD capability cell absent".into())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Configuration {
    pub fits: BTreeMap<String, Fit>,
    /// Annex-verified (or explicitly labelled scenario fixture) inventories.
    /// No fit is inferred from a ship's nationality, generation or legacy decoys.
    pub loadouts: BTreeMap<String, BTreeMap<String, u16>>,
    pub environment: Option<Environment>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub airborne_chaff:Option<airborne_chaff::Configuration>,
    #[serde(skip_serializing_if="BTreeMap::is_empty")]
    pub floating_tow_geometries:BTreeMap<String,floating_tow::FloatingTowGeometry>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Environment {
    pub source: String,
    pub sea_state: u8,
    pub wind_from_deg: f64,
    pub wind_speed_kts: f64,
}
impl Environment {
    fn validate(&self) -> Result<(), String> {
        if self.source.trim().is_empty()
            || self.sea_state > 9
            || !self.wind_from_deg.is_finite()
            || !self.wind_speed_kts.is_finite()
            || self.wind_speed_kts < 0.0
        {
            return Err("12.4: explicit source sea state/wind required".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fit {
    pub source: String,
    pub kind: ObjectKind,
    pub generation: Option<u8>,
    pub launcher: Option<Launcher>,
    /// Required for large external ACMs; 'up to seven turns' is not an exact life.
    pub endurance_s: Option<i64>,
    /// Towed ACM deployment timing is not specified by chapter12.
    pub deployment_s: Option<i64>,
    /// Reflector/cloud height is not inferred from its apparent radar size.
    pub radar_height_m: Option<f64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    LongRangeChaff,
    FloatingAsm,
    SonarJammer,
    TorpedoAcm,
    TowedTorpedoDecoy,
    MobileDecoy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Launcher {
    Internal,
    ExternalSmall,
    ExternalLarge,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Deployed,
    Expired,
    Swamped,
    Sunk,
    Destroyed,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Configuration,
    pub stocks: BTreeMap<EntityId, BTreeMap<String, u16>>,
    pub objects: BTreeMap<EntityId, Object>,
    pub last_step: i64,
    pub reported: BTreeMap<String, String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub airborne_chaff:Option<airborne_chaff::State>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Object {
    pub id: EntityId,
    pub owner: EntityId,
    pub side: Side,
    pub fit: String,
    pub source: String,
    pub kind: ObjectKind,
    pub generation: Option<u8>,
    pub status: Status,
    pub created_at: i64,
    pub ready_at: i64,
    pub expires_at: Option<i64>,
    pub pos: Vec2,
    pub depth_m: f64,
    pub course_deg: f64,
    pub speed_kts: f64,
    pub course_changes: u8,
    #[serde(default)]
    pub speed_changes:u8,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub floating_tow:Option<floating_tow::Record>,
    pub radar_signature: Option<usize>,
    pub radar_height_m: Option<f64>,
    /// A saved launch template is used only within a sensor pass, never by
    /// movement, combat, cleanup, victory or own-unit observation.
    pub template: Entity,
    pub imitated_class: Option<String>,
    pub imitated_profile: Option<super::detection2026::TargetProfile>,
}
impl Object {
    pub fn effective(&self, now: i64) -> bool {
        self.status == Status::Deployed
            && now >= self.ready_at
            && self.expires_at.is_none_or(|at| now < at)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Deploy {
        fit: String,
        at: Option<Vec2>,
        radar_signature: Option<usize>,
        course_deg: Option<f64>,
        speed_kts: Option<f64>,
        depth_m: Option<f64>,
    },
    Sink {
        object: EntityId,
    },
    MobileCourse {
        object: EntityId,
        course_deg: f64,
    },
    MobileSpeed {object:EntityId,speed_kts:f64},
    FloatingTow{object:EntityId,geometry:String},
    FloatingUntow{object:EntityId},
    StartChaffBarrier{fit:String},
    StopChaffBarrier,
}
#[derive(Clone, Debug)]
pub struct AcousticDefense {
    pub id: EntityId,
    pub generation: u8,
    pub towed: bool,
}

fn emit(w: &mut World, side: Side, unit: EntityId, rule: &str, detail: Value) {
    w.emit(
        crate::EventKind::RulesEvent {
            domain: "ew2026".into(),
            rule: rule.into(),
            side: Some(side),
            unit: Some(unit),
            contact: None,
            detail: detail.to_string(),
        },
        crate::Visibility::Side(side),
    );
}
fn finite(v: f64) -> Result<(), String> {
    if v.is_finite() {
        Ok(())
    } else {
        Err("chapter12: finite order required".into())
    }
}
fn acoustic_life(fit: &Fit) -> Result<i64, String> {
    Ok(match fit.launcher.ok_or("12.6.1: source internal/external launcher size required")? {
        Launcher::Internal => seconds("acoustic_countermeasures", "internal_stationary_endurance_s"),
        Launcher::ExternalSmall => seconds("acoustic_countermeasures", "small_external_endurance_s"),
        Launcher::ExternalLarge => fit.endurance_s.filter(|s| *s > 0 && *s <= seconds("acoustic_countermeasures", "large_external_endurance_max_s")).ok_or("12.6.1: exact source large-external life required; maximum is not an exact endurance")?,
    })
}
fn mobile_row(generation: u8) -> Result<&'static Value, String> {
    if !(1..=4).contains(&generation) {
        return Err("12.6.2: mobile decoy generation must be1..4".into());
    }
    Ok(&tables()["mobile_submarine_decoys"]["generations"][(generation - 1) as usize])
}

pub fn apply(w: &mut World, unit: EntityId, action: &Action) -> Result<(), String> {
    if !active(w) {
        return Err("chapter12 actions require explicit hv2026-rts-v4".into());
    }
    let owner = w
        .entity(unit)
        .filter(|e| e.alive)
        .cloned()
        .ok_or("chapter12: live launcher required")?;
    match action {
        Action::MobileSpeed{object,speed_kts}=>return mobile_speed::apply(w,unit,*object,*speed_kts),
        Action::FloatingTow{object,geometry}=>return floating_tow::attach(w,unit,*object,geometry),
        Action::FloatingUntow{object}=>return floating_tow::detach(w,unit,*object),
        Action::StartChaffBarrier{fit}=>return airborne_chaff::apply(w,unit,Some(fit)),
        Action::StopChaffBarrier=>return airborne_chaff::apply(w,unit,None),
        Action::Sink { object } => {
            let obj = w
                .rules
                .ew
                .as_mut()
                .and_then(|s| s.objects.get_mut(object))
                .ok_or("12.4: object absent")?;
            if obj.owner != unit
                || obj.kind != ObjectKind::FloatingAsm
                || obj.status != Status::Deployed
            {
                return Err("12.4: only own deployed floating decoy may be sunk early".into());
            }
            obj.status = Status::Sunk;
            floating_tow::terminate(obj,w.tick,"sunk");
            emit(w, owner.side, unit, "12.4.sink", json!({"object":object}));
            return Ok(());
        }
        Action::MobileCourse { object, course_deg } => {
            finite(*course_deg)?;
            let obj = w
                .rules
                .ew
                .as_mut()
                .and_then(|s| s.objects.get_mut(object))
                .ok_or("12.6.2: object absent")?;
            if obj.owner != unit || obj.kind != ObjectKind::MobileDecoy || !obj.effective(w.tick) {
                return Err("12.6.2: own effective mobile decoy required".into());
            }
            if crate::geom::angle_diff(obj.course_deg, *course_deg).abs() < 1e-9 {
                return Ok(());
            }
            let maximum = mobile_row(obj.generation.ok_or("12.6.2: missing generation")?)?
                ["course_changes_max"]
                .as_u64()
                .unwrap() as u8;
            if obj.course_changes >= maximum {
                return Err("12.6.2: source course-change allowance exhausted".into());
            }
            obj.course_deg = crate::geom::normalize_deg(*course_deg);
            obj.course_changes += 1;
            emit(
                w,
                owner.side,
                unit,
                "12.6.2.course",
                json!({"object":object,"course_deg":course_deg}),
            );
            return Ok(());
        }
        Action::Deploy { .. } => {}
    }
    let Action::Deploy {
        fit: fit_id,
        at,
        radar_signature,
        course_deg,
        speed_kts,
        depth_m,
    } = action
    else {
        unreachable!()
    };
    let state = w
        .rules
        .ew
        .as_ref()
        .ok_or("chapter12: missing v4 EW state")?;
    let fit = state
        .config
        .fits
        .get(fit_id)
        .filter(|f| !f.source.trim().is_empty())
        .cloned()
        .ok_or("chapter12: source-verified fitted countermeasure required")?;
    let stock = state
        .stocks
        .get(&unit)
        .or_else(|| state.config.loadouts.get(&owner.class))
        .and_then(|m| m.get(fit_id))
        .copied()
        .unwrap_or(0);
    if stock == 0 {
        return Err("chapter12: fitted inventory exhausted or unknown".into());
    }
    let pos = at.unwrap_or(owner.pos);
    finite(pos.x)?;
    finite(pos.y)?;
    let mut depth = depth_m.unwrap_or(owner.depth_m);
    finite(depth)?;
    if depth < 0.0 {
        return Err("chapter12: nonnegative depth required".into());
    }
    let mut course = course_deg.unwrap_or(owner.course_deg);
    finite(course)?;
    let speed = speed_kts.unwrap_or(0.0);
    finite(speed)?;
    if speed < 0.0 {
        return Err("chapter12: nonnegative speed required".into());
    }
    let mut signature = *radar_signature;
    let (delay, life): (i64, Option<i64>);
    let mut status = Status::Deployed;
    let mut imitated_profile = None;
    let mut imitated_class = None;
    match fit.kind {
        ObjectKind::LongRangeChaff => {
            if owner.kind != PlatformKind::Ship
                || owner.pos.range_to(pos) > number("long_range_chaff", "max_range_nm")
            {
                return Err("12.3: ship launch within5nmi required".into());
            }
            if radar_signature.is_some() || speed != 0.0 || depth_m.is_some_and(|d| d != 0.0) {
                return Err("12.3: chaff signature must be rolled, not selected; cloud is stationary at surface".into());
            }
            signature = None;
            depth = 0.;
            delay = 0;
            life = Some(seconds("long_range_chaff", "lifetime_s"));
            fit.radar_height_m.filter(|h|h.is_finite()&&*h>=0.).ok_or("12.3: explicit source cloud/reflector height required, not a ship height guessed from echo size")?;
        }
        ObjectKind::FloatingAsm => {
            if owner.kind != PlatformKind::Ship
                || owner.pos.range_to(pos) > 1e-9
                || !matches!(signature, Some(1 | 2))
                || speed != 0.0
                || depth_m.is_some_and(|d| d != 0.)
            {
                return Err(
                    "12.4: ship-side surface deployment with Small/Medium echo required".into(),
                );
            }
            let env = state
                .config
                .environment
                .as_ref()
                .ok_or("12.4: source sea state and wind required")?;
            env.validate()?;
            fit.radar_height_m.filter(|h|h.is_finite()&&*h>=0.).ok_or("12.4: explicit source reflector height required, not a ship height guessed from echo size")?;
            if env.sea_state as f64 > number("floating_asm_decoy", "max_sea_state") {
                status = Status::Swamped;
            }
            depth = 0.;
            delay = seconds("floating_asm_decoy", "deployment_s");
            life = Some(seconds("floating_asm_decoy", "lifetime_max_s"));
        }
        ObjectKind::SonarJammer | ObjectKind::TorpedoAcm => {
            if !matches!(owner.kind, PlatformKind::Ship | PlatformKind::Sub)
                || owner.pos.range_to(pos) > 1e-9
                || speed != 0.0
                || signature.is_some()
                || depth_m.is_some_and(|d| (d - owner.depth_m).abs() > 1e-9)
            {
                return Err("12.6.1: stationary ship/sub local-depth launched ACM required; short-range mobile jammer profile unresolved".into());
            }
            if fit.kind == ObjectKind::TorpedoAcm
                && !fit.generation.is_some_and(|g| (1..=4).contains(&g))
            {
                return Err("12.6.1: source torpedo ACM generation1..4 required".into());
            }
            delay = 0;
            life = Some(acoustic_life(&fit)?);
        }
        ObjectKind::TowedTorpedoDecoy => {
            if owner.kind != PlatformKind::Ship
                || owner.pos.range_to(pos) > 1e-9
                || signature.is_some()
                || !fit.generation.is_some_and(|g| (1..=4).contains(&g))
            {
                return Err("12.6.1: source surface-ship towed ACM generation required".into());
            }
            delay = fit
                .deployment_s
                .filter(|d| *d >= 0)
                .ok_or("12.6.1: source fitted towed-decoy deployment time required")?;
            life = None;
        }
        ObjectKind::MobileDecoy => {
            if owner.kind != PlatformKind::Sub
                || owner.pos.range_to(pos) > 1e-9
                || signature.is_some()
            {
                return Err("12.6.2: submarine-local launch required".into());
            }
            let row = mobile_row(fit.generation.ok_or("12.6.2: source generation required")?)?;
            let maximum = row["max_speed_kts"].as_f64().unwrap();
            if (speed - maximum).abs() > 1e-9 {
                return Err("12.6.2: only source maximum-speed endurance is resolved; no low-speed energy extrapolation".into());
            }
            if course_deg.is_none() || depth_m.is_none() {
                return Err("12.6.2: launch course and depth must be plotted explicitly".into());
            }
            let water = w
                .rules
                .detection
                .as_ref()
                .and_then(|s| s.config.environment.as_ref())
                .map(|e| e.water_depth_m)
                .ok_or("12.6.2: source scenario water depth required")?;
            if depth > water {
                return Err("12.6.2: plotted depth below sea floor".into());
            }
            let profile = w
                .rules
                .detection
                .as_ref()
                .and_then(|s| s.config.targets.get(&owner.class))
                .filter(|p| !p.source.trim().is_empty() && p.noise.is_some())
                .cloned()
                .ok_or("12.6.2: source submarine signature to imitate required")?;
            imitated_profile = Some(profile);
            imitated_class = Some(owner.class.clone());
            delay = 0;
            life = Some(row["endurance_s"].as_i64().unwrap());
        }
    }
    course = crate::geom::normalize_deg(course);
    let id = w.allocate_id();
    let mut d100 = None;
    if fit.kind == ObjectKind::LongRangeChaff {
        let mut rng = w.scope_rng(id, "12.3.initial_signature");
        let roll = (rng.next_f64() * 100.) as u8 + 1;
        let small_chance = tables()["long_range_chaff"]["signature_distribution"][0]
            ["chance_percent"]
            .as_u64()
            .unwrap() as u8;
        signature = Some(if roll <= small_chance { 2 } else { 1 });
        d100 = Some(roll);
    }
    let state = w.rules.ew.as_mut().unwrap();
    let stocks = state.stocks.entry(unit).or_insert_with(|| {
        state
            .config
            .loadouts
            .get(&owner.class)
            .cloned()
            .unwrap_or_default()
    });
    *stocks.get_mut(fit_id).expect("validated stock") -= 1;
    let mut template = owner.clone();
    template.id = id;
    template.class = format!("__ew_object_{}", id.0);
    template.name = "EW object".into();
    template.rule_data = None;
    template.weapons.clear();
    template.sensors.clear();
    template.deck = None;
    template.mission = None;
    template.station = None;
    template.projectile = None;
    template.waypoints.clear();
    template.ext.clear();
    template.ext.insert("ew.virtual".into(), 1.0);
    template.last_emit_tick = 0;
    template.fire_on = false;
    template.auto_chaff = false;
    template.chaff = 0;
    template.decoys = 0;
    template.endurance_s = None;
    template.burn_s = 0.;
    template.flooding_s = 0.;
    template.hp = 1.;
    template.hp_max = 1.;
    template.assigned = None;
    template.kind = if fit.kind == ObjectKind::MobileDecoy {
        PlatformKind::Sub
    } else {
        PlatformKind::Ship
    };
    state.objects.insert(
        id,
        Object {
            id,
            owner: unit,
            side: owner.side,
            fit: fit_id.clone(),
            source: fit.source.clone(),
            kind: fit.kind,
            generation: fit.generation,
            status,
            created_at: w.tick,
            ready_at: w.tick + delay,
            expires_at: life.map(|s| w.tick + delay + s),
            pos,
            depth_m: depth,
            course_deg: course,
            speed_kts: speed,
            course_changes: 0,
            speed_changes: 0,
            floating_tow:None,
            radar_signature: signature,
            radar_height_m: fit.radar_height_m,
            template,
            imitated_class,
            imitated_profile,
        },
    );
    emit(
        w,
        owner.side,
        unit,
        "12.deploy",
        json!({"object":id,"fit":fit_id,"source":fit.source,"status":status,"ready_at":w.tick+delay,"expires_at":life.map(|s|w.tick+delay+s),"signature_d100":d100,"radar_signature":signature}),
    );
    Ok(())
}

/// Actual world step. Source-defined expiry and drift use saved object clocks;
/// reloading cannot restore life or roll a new false-echo signature.
pub fn step(w: &mut World) {
    if !active(w) {
        return;
    }
    let Some(mut state) = w.rules.ew.take() else {
        return;
    };
    let mut evidence = Vec::new();
    let last = state.last_step;
    let now = w.tick;
    for obj in state
        .objects
        .values_mut()
        .filter(|o| o.status == Status::Deployed)
    {
        let through = obj.expires_at.map_or(now, |at| now.min(at));
        if obj.kind == ObjectKind::FloatingAsm {
            if let Some(env) = state
                .config
                .environment
                .as_ref()
                .filter(|e| e.validate().is_ok())
            {
                if env.sea_state as f64 > number("floating_asm_decoy", "max_sea_state") {
                    obj.status = Status::Swamped;
                    floating_tow::terminate(obj,now,"swamped");
                    evidence.push((
                        obj.side,
                        obj.owner,
                        "12.4.swamped",
                        json!({"object":obj.id,"sea_state":env.sea_state}),
                    ));
                    continue;
                }
                let free=match floating_tow::step_one(w,obj,now){
                    floating_tow::TowStep::Free=>true,
                    floating_tow::TowStep::Followed|floating_tow::TowStep::Pending=>false,
                    floating_tow::TowStep::Ended{rule,detail}=>{
                        evidence.push((obj.side,obj.owner,rule,detail));true
                    }
                };
                if free {
                    let elapsed = (through - last.max(obj.ready_at)).max(0) as f64;
                    obj.pos = obj.pos.advanced(
                        env.wind_from_deg + 180.,
                        env.wind_speed_kts
                            * number("floating_asm_decoy", "wind_drift_speed_fraction")
                            * elapsed
                            / 3600.,
                    );
                }
            }
        } else if obj.kind == ObjectKind::MobileDecoy {
            let elapsed = (through - last.max(obj.ready_at)).max(0) as f64;
            obj.pos = obj
                .pos
                .advanced(obj.course_deg, obj.speed_kts * elapsed / 3600.);
        } else if obj.kind == ObjectKind::TowedTorpedoDecoy {
            if let Some(owner) = w.entity(obj.owner).filter(|e| e.alive) {
                obj.pos = owner.pos;
                obj.depth_m = owner.depth_m;
            } else {
                obj.status = Status::Destroyed;
                evidence.push((
                    obj.side,
                    obj.owner,
                    "12.6.tow_owner_lost",
                    json!({"object":obj.id}),
                ));
            }
        }
        if obj.status == Status::Deployed && obj.expires_at.is_some_and(|at| now >= at) {
            obj.status = Status::Expired;
            floating_tow::terminate(obj,now,"expired");
            evidence.push((obj.side, obj.owner, "12.expired", json!({"object":obj.id})));
        }
    }
    state.last_step = now;
    w.rules.ew = Some(state);
    for (side, unit, rule, detail) in evidence {
        emit(w, side, unit, rule, detail);
    }
    airborne_chaff::step(w);
}

pub fn active_acms(w: &World, target: EntityId) -> Vec<AcousticDefense> {
    if !active(w) {
        return Vec::new();
    }
    w.rules
        .ew
        .as_ref()
        .map(|s| {
            s.objects
                .values()
                .filter(|o| {
                    o.owner == target
                        && o.effective(w.tick)
                        && matches!(
                            o.kind,
                            ObjectKind::TorpedoAcm
                                | ObjectKind::TowedTorpedoDecoy
                                | ObjectKind::MobileDecoy
                        )
                })
                .filter_map(|o| {
                    o.generation.map(|g| AcousticDefense {
                        id: o.id,
                        generation: g,
                        towed: o.kind == ObjectKind::TowedTorpedoDecoy,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
pub fn consume_towed(w: &mut World, id: EntityId) {
    let now = w.tick;
    let Some(o) = w
        .rules
        .ew
        .as_mut()
        .and_then(|s| s.objects.get_mut(&id))
        .filter(|o| o.kind == ObjectKind::TowedTorpedoDecoy && o.effective(now))
    else {
        return;
    };
    o.status = Status::Destroyed;
    let (side, owner) = (o.side, o.owner);
    emit(
        w,
        side,
        owner,
        "12.6.towed_seduced_destroyed",
        json!({"object":id}),
    );
}
pub fn object(w: &World, id: EntityId) -> Option<&Object> {
    w.rules.ew.as_ref()?.objects.get(&id)
}
pub fn is_object(w: &World, id: EntityId) -> bool {
    object(w, id).is_some()
}
pub fn radar_signature(w: &World, id: EntityId) -> Option<usize> {
    object(w, id)
        .filter(|o| o.effective(w.tick))?
        .radar_signature
}

/// Sensor-only views have no life outside update_sensors_v4. In particular they
/// never contribute to movement, losses, victory points or real platform fits.
pub fn sensor_targets(w: &World) -> Vec<Entity> {
    w.rules
        .ew
        .as_ref()
        .map(|s| {
            s.objects
                .values()
                .filter(|o| {
                    o.effective(w.tick)
                        && matches!(
                            o.kind,
                            ObjectKind::LongRangeChaff
                                | ObjectKind::FloatingAsm
                                | ObjectKind::SonarJammer
                                | ObjectKind::MobileDecoy
                        )
                })
                .map(|o| {
                    let mut e = o.template.clone();
                    e.pos = o.pos;
                    e.depth_m = o.depth_m;
                    e.ordered_depth_m = o.depth_m;
                    e.altitude_m = 0.;
                    e.ordered_altitude_m = 0.;
                    e.course_deg = o.course_deg;
                    e.ordered_course_deg = o.course_deg;
                    e.speed_kts = o.speed_kts;
                    e.ordered_speed_kts = o.speed_kts;
                    e
                })
                .collect()
        })
        .unwrap_or_default()
}
pub fn own_view(w: &World, side: Side, unit: EntityId) -> Option<Value> {
    if !active(w) || w.entity(unit)?.side != side {
        return None;
    }
    let s = w.rules.ew.as_ref()?;
    let stocks = s
        .stocks
        .get(&unit)
        .or_else(|| s.config.loadouts.get(&w.entity(unit)?.class));
    let objects:Vec<_>=s.objects.values().filter(|o|o.owner==unit).map(|o|json!({"id":o.id,"fit":o.fit,"kind":o.kind,"status":o.status,"ready_at":o.ready_at,"expires_at":o.expires_at,"pos":o.pos,"depth_m":o.depth_m,"course_deg":o.course_deg,"speed_kts":o.speed_kts,"course_changes":o.course_changes,"speed_changes":o.speed_changes,"floating_tow":o.floating_tow})).collect();
    Some(json!({"stocks":stocks,"objects":objects,"airborne_chaff":airborne_chaff::own_view(w,side,unit)}))
}

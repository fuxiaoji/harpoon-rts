//! Harpoon V2026 §6.3 RF/acoustic/visual fire-control source executor, opt-in v4.
//! The source describes four-Tactical-Turn loss tolerance; observer provenance is retained separately from the side contact book.
use crate::{db::SensorKind, geom::angle_diff, types::PlatformKind, EntityId, Side, Vec2, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::OnceLock};
pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/firecontrol-2026.json")).unwrap()
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Quality {
    NA,
    Poor,
    Fair,
    Good,
}
impl Quality {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "NA" => Ok(Self::NA),
            "Poor" => Ok(Self::Poor),
            "Fair" => Ok(Self::Fair),
            "Good" => Ok(Self::Good),
            _ => Err("6.3 unknown source solution quality".into()),
        }
    }
    pub fn lower(self) -> Self {
        match self {
            Self::Good => Self::Fair,
            Self::Fair => Self::Poor,
            _ => Self::NA,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    ActiveRf,
    PassiveRf,
    ActiveAcoustic,
    PassiveAcoustic,
    Visual,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub tracks: BTreeMap<String, Track>,
    pub placements: BTreeMap<String, Placement>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub observer: EntityId,
    pub target: EntityId,
    pub sensor: String,
    pub first: i64,
    pub last: i64,
    pub turn_course: f64,
    pub turn_speed: f64,
    pub last_turn: i64,
    pub zig_until: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<super::detection2026::SensorReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<OwnMotion>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnMotion {
    pub start: i64,
    pub last: i64,
    pub first: Vec2,
    pub known: bool,
    pub course_deg: Option<f64>,
    pub speed_kts: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub kind: Kind,
    pub track_turns: i32,
    pub range_nm: f64,
    pub contact_speed: f64,
    pub combat_generation: usize,
    pub combat_aircraft: bool,
    pub oth_targeting: bool,
    pub aircraft_rf: bool,
    pub seeker_generation: usize,
    pub weapon_speed_kts: f64,
    pub long_range_seeker: bool,
    pub scout: bool,
    pub tdl_cue: u8,
    pub decoy: bool,
    pub crossfix_generation: Option<usize>,
    pub direct_visual_ranging: bool,
    pub passive_array: Option<String>,
    pub zig: bool,
    pub cz: Option<u8>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Solution {
    pub quality: Quality,
    pub contact_track: i32,
    pub range_column: usize,
    pub modifiers: BTreeMap<String, i32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Placement {
    pub identity: String,
    pub tick: i64,
    pub d10: u8,
    pub chance: i32,
    pub placed: bool,
    pub deviation_d6: [u8; 2],
    pub course_deviation_deg: i32,
}
fn col(limits: &Value, v: f64) -> Result<usize, String> {
    if !v.is_finite() || v < 0. {
        return Err("6.3 invalid/unknown input".into());
    }
    limits
        .as_array()
        .unwrap()
        .iter()
        .position(|n| n.is_null() || v <= n.as_f64().unwrap())
        .ok_or_else(|| "6.3 outside printed range columns".into())
}
fn number(v: &Value) -> Result<i32, String> {
    v.as_i64()
        .map(|n| n as i32)
        .ok_or_else(|| "6.3 required source field absent".into())
}
pub fn solve(i: &Input) -> Result<Solution, String> {
    solve_inner(i, false)
}
/// §8.5.3 uses the acoustic tracking table without a torpedo seeker.
/// The otherwise unused seeker slot must be zero, not a fictitious Gen2.
pub fn solve_projector(i: &Input) -> Result<Solution, String> {
    if i.kind != Kind::ActiveAcoustic || i.seeker_generation != 0 {
        return Err("8.5.3 initial projector scope requires active acoustic FCS and no seeker".into());
    }
    solve_inner(i, true)
}
fn solve_inner(i: &Input, projector: bool) -> Result<Solution, String> {
    if i.passive_array.is_some() && i.kind != Kind::PassiveAcoustic {
        return Err("6.3.4 passive ranging array requires passive acoustic fire control".into());
    }
    if !(1..=6).contains(&i.combat_generation)
        || (!projector && !(1..=4).contains(&i.seeker_generation))
        || i.tdl_cue > 2
    {
        return Err("6.3 unknown source generation/TDL".into());
    }
    let rf = matches!(i.kind, Kind::ActiveRf | Kind::PassiveRf);
    let active = matches!(i.kind, Kind::ActiveRf | Kind::ActiveAcoustic | Kind::Visual);
    let mut c = match i.kind {
        Kind::ActiveRf | Kind::PassiveRf => col(&tables()["rf_limits_nm"], i.range_nm)?,
        Kind::ActiveAcoustic => match i.cz {
            Some(1) => 5,
            Some(2) => 6,
            Some(_) => return Err("6.3 no third-CZ FCS column".into()),
            None => col(&tables()["acoustic_active_limits_nm"], i.range_nm)?,
        },
        Kind::Visual => col(&tables()["acoustic_active_limits_nm"], i.range_nm)?,
        Kind::PassiveAcoustic => {
            if i.cz.is_some() {
                return Err("6.3 passive CZ FCS impossible".into());
            }
            col(&tables()["acoustic_passive_limits_nm"], i.range_nm)?
        }
    };
    let original_column = c;
    let mut mods = BTreeMap::new();
    mods.insert(
        "combat_system".into(),
        if i.combat_aircraft && rf {
            0
        } else {
            number(
                &tables()[if active {
                    "combat_active"
                } else {
                    "combat_passive"
                }][i.combat_generation - 1],
            )?
        },
    );
    mods.insert(
        "contact_speed".into(),
        number(
            &tables()[if active {
                "speed_active"
            } else {
                "speed_passive"
            }][col(&tables()["speed_limits_kts"], i.contact_speed)?],
        )?,
    );
    mods.insert("tdl".into(), i.tdl_cue as i32);
    mods.insert(
        "decoy".into(),
        if i.decoy {
            if active {
                -2
            } else {
                -3
            }
        } else {
            0
        },
    );
    let mut cap = Quality::Good;
    if rf {
        if i.aircraft_rf {
            c = c.saturating_sub(1)
        }
        if !i.oth_targeting && !i.aircraft_rf {
            let row = &tables()[if active {
                "non_oth_active"
            } else {
                "non_oth_passive"
            }][col(
                &serde_json::to_value(
                    tables()[if active {
                        "non_oth_active"
                    } else {
                        "non_oth_passive"
                    }]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r[0].clone())
                    .collect::<Vec<_>>(),
                )
                .unwrap(),
                i.range_nm,
            )?];
            cap = Quality::parse(row[1].as_str().unwrap())?;
            if cap == Quality::NA {
                return Ok(Solution {
                    quality: cap,
                    contact_track: i.track_turns,
                    range_column: c,
                    modifiers: mods,
                });
            }
            mods.insert("not_oth".into(), number(&row[2])?);
        }
        if !projector { mods.insert(
            "ascm_seeker".into(),
            number(&tables()["ascm_seeker"][i.seeker_generation - 1])?,
        );
        mods.insert(
            "ascm_speed".into(),
            number(
                &tables()["ascm_speed"]
                    [col(&tables()["ascm_speed_limits_kts"], i.weapon_speed_kts)?][original_column],
            )?,
        );
        mods.insert(
            "long_range_scout".into(),
            if i.long_range_seeker {
                if i.scout {
                    2
                } else {
                    1
                }
            } else {
                0
            },
        ); }
    } else {
        if !projector { mods.insert(
            "torpedo_seeker".into(),
            if i.kind == Kind::Visual {
                0
            } else {
                number(&tables()["torpedo_seeker"][i.seeker_generation - 1])?
            },
        ); }
        if i.kind == Kind::Visual && i.direct_visual_ranging {
            c = c.saturating_sub(1)
        }
        if let Some(a) = &i.passive_array {
            if i.range_nm > 12. {
                return Err("6.3.4 passive ranging array outside12nm".into());
            }
            mods.insert(
                "passive_array".into(),
                match a.as_str() {
                    "wavefront" => 4,
                    "vertical_flank" => 3,
                    _ => return Err("6.3 source passive ranging method missing".into()),
                },
            );
        }
        mods.insert("target_zig".into(), if i.zig { -2 } else { 0 });
    }
    let contact_track = i.track_turns + mods.values().sum::<i32>();
    let table = match i.kind {
        Kind::ActiveRf => "active_rf",
        // §6.3.3: ES cross-fixes use the Active RF table, then apply
        // the generation's quality adjustment. A passive-table lookup
        // followed by that adjustment understates all three source cases.
        Kind::PassiveRf if i.crossfix_generation.is_some() => "active_rf",
        Kind::PassiveRf => "passive_rf",
        Kind::ActiveAcoustic | Kind::Visual => "active_acoustic",
        Kind::PassiveAcoustic => "passive_acoustic",
    };
    let rows = tables()[table].as_array().unwrap();
    let row = rows
        .iter()
        .filter(|r| r[0].as_i64().unwrap() <= contact_track as i64)
        .last()
        .unwrap_or(&rows[0]);
    let mut q = Quality::parse(row[c + 1].as_str().unwrap())?.min(cap);
    if let Some(g) = i.crossfix_generation {
        if !rf {
            return Err("6.3.3 is RF cross-fix, not sonar geometry bonus".into());
        }
        q = match g {
            1 | 2 => q.lower(),
            3 | 4 => q,
            5 | 6 => match q {
                Quality::NA => Quality::Poor,
                Quality::Poor => Quality::Fair,
                _ => Quality::Good,
            },
            _ => return Err("6.3.3 crossfix ES generation absent".into()),
        }
        .min(cap);
    }
    Ok(Solution {
        quality: q,
        contact_track,
        range_column: c,
        modifiers: mods,
    })
}
/// Consume actual successful detection after the side contact has been updated. A fused book alone never creates an observer track.
pub fn record_detection(w: &mut World, report: &super::detection2026::SensorReport) {
    if !super::aaw2026::active(w) {
        return;
    }
    let (observer, target, sensor) = (report.observer, report.target, report.sensor.as_str());
    let Some(o) = w.entity(observer).filter(|e| e.alive) else {
        return;
    };
    let side = o.side;
    if !w.sides[side.index()]
        .contacts
        .values()
        .any(|c| c.about == target && !c.stale)
    {
        return;
    }
    if report.at != w.tick || !o.sensors.iter().any(|s| s.sensor == sensor) {
        return;
    }
    let key = format!("{}:{sensor}:{}", observer.0, target.0);
    let now = w.tick;
    let Some(aaw) = w.rules.aaw.as_mut() else {
        return;
    };
    let t = aaw.firecontrol.tracks.entry(key).or_insert(Track {
        observer,
        target,
        sensor: sensor.into(),
        first: now,
        last: now,
        turn_course: 0.,
        turn_speed: 0.,
        last_turn: now.div_euclid(180),
        zig_until: 0,
        report: None,
        motion: None,
    });
    if now - t.last >= 720 {
        t.first = now;
    }
    let policy = &tables()["execution_revision"]["own_motion"];
    let window = policy["window_s"].as_i64().unwrap();
    let gap = policy["maximum_sample_gap_s"].as_i64().unwrap();
    if let Some(position) = report.measured_position {
        let motion = t.motion.get_or_insert(OwnMotion {
            start: now,
            last: now,
            first: position,
            known: false,
            course_deg: None,
            speed_kts: None,
        });
        if now - motion.last > gap {
            *motion = OwnMotion {
                start: now,
                last: now,
                first: position,
                known: false,
                course_deg: None,
                speed_kts: None,
            };
        }
        motion.last = now;
        if now - motion.start >= window {
            let distance = motion.first.range_to(position);
            let course = (distance > 0.).then(|| motion.first.bearing_to(position));
            let speed = distance * 3600. / (now - motion.start) as f64;
            if motion.known
                && (motion
                    .course_deg
                    .zip(course)
                    .is_some_and(|(a, b)| angle_diff(a, b).abs() >= 20.)
                    || motion
                        .speed_kts
                        .is_some_and(|old| (speed - old).abs() >= 5.))
            {
                t.zig_until = (now.div_euclid(180) + 1) * 180;
            }
            motion.known = true;
            motion.course_deg = course;
            motion.speed_kts = Some(speed);
            motion.start = now;
            motion.first = position;
            t.turn_course = course.unwrap_or(0.);
            t.turn_speed = speed;
            t.last_turn = now.div_euclid(180);
        }
    } else {
        t.motion = None;
    }
    t.report = Some(report.clone());
    t.last = now;
}
/// Single-platform source FCS. TDL/cross-fix is not inferred from a shared side contact.
pub fn own_world_solution(
    w: &World,
    actor: EntityId,
    target: EntityId,
    seeker: usize,
    weapon_speed: f64,
) -> Result<Solution, String> {
    own_filtered_solution(w,actor,target,None,seeker,weapon_speed,false,false)
}
/// A transmission's quality and aim must come from the same native sensor.
/// Received solutions never participate in forming another transmission.
pub fn own_sensor_solution(w:&World,actor:EntityId,target:EntityId,sensor:&str,seeker:usize,weapon_speed:f64)->Result<Solution,String>{
    own_filtered_solution(w,actor,target,Some(sensor),seeker,weapon_speed,false,false)
}
/// Initial projector scope: an actual Gen4+ active search receiver, measured
/// position and continuous native motion. No inherited torpedo seeker bonus.
pub fn own_projector_solution(w:&World,actor:EntityId,target:EntityId,sensor:&str)->Result<Solution,String>{
    own_filtered_solution(w,actor,target,Some(sensor),0,0.,true,false)
}
/// Native ballistic-ASW placement quality has no ASCM or torpedo seeker.
/// Unlike the first projector domain, qualified passive/RF sources are legal.
pub fn own_ballistic_asw_solution(w:&World,actor:EntityId,target:EntityId,sensor:&str)->Result<Solution,String>{
    own_filtered_solution(w,actor,target,Some(sensor),0,0.,true,true)
}
/// Aircraft RF fire control for guided surface ordnance. The source RF
/// solver applies no ASCM seeker-generation or missile-speed modifier here.
pub fn own_air_surface_rf_solution(w:&World,actor:EntityId,target:EntityId,sensor:&str)->Result<Solution,String>{
    let own=w.entity(actor).filter(|e|e.alive&&e.kind==PlatformKind::Aircraft).ok_or("6.3 source air RF observer unavailable")?;
    if !own.sensors.iter().any(|s|s.sensor==sensor)
        || !w.db.sensors.get(sensor).is_some_and(|s|matches!(s.kind,SensorKind::Radar|SensorKind::Esm)) {
        return Err("6.3 guided air surface RF solution requires its own fitted Radar/ES receiver".into());
    }
    own_filtered_solution(w,actor,target,Some(sensor),0,0.,true,true)
}
fn own_filtered_solution(w:&World,actor:EntityId,target:EntityId,sensor:Option<&str>,seeker:usize,weapon_speed:f64,projector:bool,ballistic:bool)->Result<Solution,String>{
    let e = w
        .entity(actor)
        .filter(|e| e.alive)
        .ok_or("6.3 shooter missing")?;
    let _contact = w.sides[e.side.index()]
        .contacts
        .values()
        .find(|c| c.about == target && !c.stale)
        .ok_or("6.3 own contact absent")?;
    let legacy_cs = super::operations::tables()["aaw"]["combat_systems"]
        .get(&e.class)
        .and_then(|v| v[0].as_u64())
        .map(|n| n as usize);
    let state = &w
        .rules
        .aaw
        .as_ref()
        .ok_or("source fire-control runtime missing")?
        .firecontrol;
    let mut best = None;
    let mut missing_generation = false;
    for t in state
        .tracks
        .values()
        .filter(|t| t.observer == actor && t.target == target && w.tick - t.last < 720 && sensor.is_none_or(|s|s==t.sensor))
    {
        if !super::detection2026::array_optional_world::fire_control_allowed(w,actor,&t.sensor,target){continue;}
        let fresh = tables()["execution_revision"]["own_motion"]["report_freshness_s"]
            .as_i64()
            .unwrap();
        let Some(report) = t.report.as_ref().filter(|r| {
            r.observer == actor
                && r.target == target
                && r.sensor == t.sensor
                && r.at <= w.tick
                && w.tick - r.at <= fresh
        }) else {
            continue;
        };
        let Some(position) = report.measured_position else {
            continue;
        };
        let Some(motion) = t.motion.as_ref().filter(|m| m.known && m.last == report.at) else {
            continue;
        };
        let Some(speed) = motion.speed_kts else {
            continue;
        };
        let Some(d) = w.db.sensors.get(&t.sensor) else {
            continue;
        };
        if projector && !ballistic && (e.systems.sonar <= 0. || d.kind != SensorKind::SonarActive
            || !(matches!(&report.receiver,Some(super::detection2026::Receiver::Sonar{source,generation:4..=7,..}) if !source.trim().is_empty())
                || super::asw_weapons2026::projector::attack_sonar::fcs_allowed(w,actor,target,&t.sensor,report))
            || !e.sensors.iter().any(|s|s.sensor==t.sensor && s.is_active_kind(SensorKind::SonarActive))) {
            continue;
        }
        if crate::sensors::sensor_damaged(e, &d.id) {
            continue;
        }
        if !super::expanded_damage2026::sensor_operational(w,actor,&d.id).unwrap_or(false){continue;}
        if !e.sensors.iter().any(|s| s.sensor == t.sensor) || report.sensor_kind != d.kind {
            continue;
        }
        let (kind, oth, aircraft_rf) = match d.kind {
            SensorKind::Radar => {
                let Some(p) = super::aviation2026::operations_radar_profile(w, e, d) else {
                    continue;
                };
                (
                    Kind::ActiveRf,
                    p["role"] == "SS-T",
                    e.kind == PlatformKind::Aircraft,
                )
            }
            SensorKind::Esm => (Kind::PassiveRf, false, e.kind == PlatformKind::Aircraft),
            SensorKind::SonarActive => (Kind::ActiveAcoustic, false, false),
            SensorKind::SonarPassive => (Kind::PassiveAcoustic, false, false),
            SensorKind::Visual | SensorKind::Ir => (Kind::Visual, false, false),
            _ => continue,
        };
        let acoustic_cs = if e.kind == PlatformKind::Sub
            && matches!(kind, Kind::ActiveAcoustic | Kind::PassiveAcoustic)
        {
            let row = &tables()["platform_combat_systems"][&e.class];
            (row["scope"] == "acoustic")
                .then(|| row["generation"].as_u64().map(|n| n as usize))
                .flatten()
        } else {
            None
        };
        // Aircraft RF explicitly omits the combat-system modifier in §6.3.
        // Its unused solver slot is not a Gen1 platform binding. Acoustic and
        // visual solutions consume that modifier and require an actual source.
        let cs = acoustic_cs.or(legacy_cs).or_else(|| {
            (e.kind == PlatformKind::Aircraft && matches!(kind, Kind::ActiveRf | Kind::PassiveRf))
                .then_some(1)
        });
        let Some(cs) = cs else {
            missing_generation = true;
            continue;
        };
        let i = Input {
            kind,
            track_turns: ((w.tick - t.first) / 180) as i32,
            range_nm: e.pos.range_to(position),
            contact_speed: speed,
            combat_generation: cs,
            combat_aircraft: e.kind == PlatformKind::Aircraft,
            oth_targeting: oth,
            aircraft_rf,
            seeker_generation: seeker,
            weapon_speed_kts: weapon_speed,
            long_range_seeker: false,
            scout: false,
            tdl_cue: match kind {
                Kind::ActiveAcoustic|Kind::PassiveAcoustic=>super::communications2026::cue_evidence(w,actor,target,super::communications2026::CuePurpose::Acoustic).modifier,
                Kind::ActiveRf|Kind::PassiveRf=>super::communications2026::cue_evidence(w,actor,target,super::communications2026::CuePurpose::Rf).modifier,
                Kind::Visual=>0,
            },
            decoy: false,
            crossfix_generation: None,
            direct_visual_ranging: false,
            passive_array: None,
            zig: w.tick < t.zig_until,
            cz: None,
        };
        if let Ok(s) = if ballistic {solve_inner(&i,true)} else if projector {solve_projector(&i)} else {solve(&i)} {
            if best
                .as_ref()
                .is_none_or(|old: &Solution| s.quality > old.quality)
            {
                best = Some(s)
            }
        }
    }
    best.ok_or_else(|| {
        if missing_generation {
            "6.3 verified combat system generation absent".into()
        } else {
            "6.3 no source observer track supports this attack; TDL provenance not bound".into()
        }
    })
}
/// Shared attack solution entry; the native-only entry above is used when
/// freezing a transmission so a received solution cannot become new evidence.
pub fn world_solution(w:&World,actor:EntityId,target:EntityId,seeker:usize,weapon_speed:f64)->Result<Solution,String>{
    let native=own_world_solution(w,actor,target,seeker,weapon_speed);
    let remote=match super::communications2026::received_solution(w,actor,target){Ok(r)=>r,Err(e)=>return native.or(Err(e))};
    match (native,remote.and_then(|e|e.solution)) {
        (Ok(n),Some(r))=>Ok(if r.quality>n.quality{r}else{n}),
        (Ok(n),None)=>Ok(n),(Err(_),Some(r))=>Ok(r),(Err(e),None)=>Err(e),
    }
}
/// Attack consumers can use a source-combined NA/Poor solution without
/// inventing a Contact Track number that the non-TDL rule never supplies.
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct AttackSolution {pub quality:Quality,pub native:Option<Solution>,pub reported_aim:Option<Vec2>,pub source:Value}
pub fn attack_solution(w:&World,actor:EntityId,target:EntityId,seeker:usize,weapon_speed:f64)->Result<AttackSolution,String>{
    let ordinary=attack_solution_without_buoys(w,actor,target,seeker,weapon_speed);
    if let Some(solution)=super::detection2026::sonobuoys::attack_solution(w,actor,target)
        .filter(|solution|ordinary.as_ref().map_or(true,|own|solution.quality>own.quality)) {
        return Ok(solution);
    }
    ordinary
}
fn attack_solution_without_buoys(w:&World,actor:EntityId,target:EntityId,seeker:usize,weapon_speed:f64)->Result<AttackSolution,String>{
    if let Some(mad)=super::detection2026::wide_mad::good_fix(w,actor,target,None) {
        return Ok(AttackSolution{quality:Quality::Good,native:None,reported_aim:mad.report.measured_position,
            source:json!({"rule":"5.7.1/5.7.2","source":mad.source,"observer":actor,
                "sensor":mad.report.sensor,"at":mad.report.at,"usable_turn":mad.usable_turn})});
    }
    let own=own_world_solution(w,actor,target,seeker,weapon_speed);
    let remote=match super::communications2026::received_solution(w,actor,target){Ok(r)=>r,Err(e)=>{
        return own.map(|q|AttackSolution{quality:q.quality,native:Some(q),reported_aim:None,source:json!({"rule":"6.3","source":"own native observer reports","remote_pending":e})});
    }};
    if let Some(r)=remote.filter(|r|own.as_ref().map_or(true,|q|r.quality>q.quality)) {
        return Ok(AttackSolution{quality:r.quality,native:r.solution,reported_aim:r.aim,source:json!({"rule":"6.3.9/10","formed_at":r.formed_at,"observer":r.source_observer,"sensor":r.source_sensor,"domain":r.domain,"refs":r.refs,"source":r.source})});
    }
    let q=own?;Ok(AttackSolution{quality:q.quality,native:Some(q),reported_aim:None,source:json!({"rule":"6.3","source":"own native observer reports; qualified TDL cue only"})})
}
pub fn placement(
    w: &World,
    state: &mut State,
    identity: &str,
    quality: Quality,
    modifier: i32,
) -> Result<Placement, String> {
    if let Some(p) = state.placements.get(identity) {
        return Ok(p.clone());
    }
    if quality == Quality::NA {
        return Err("6.3 no attack fire-control solution".into());
    }
    let quality = match quality {
        Quality::Good => "Good",
        Quality::Fair => "Fair",
        Quality::Poor => "Poor",
        Quality::NA => unreachable!(),
    };
    let chance = number(&tables()["placement_quality"][quality])? + modifier;
    let mut rng = w.rng.derive(crate::Rng::hash_tag(0, 0, identity));
    let d10 = 1 + (rng.next_f64() * 10.) as u8;
    let direction = 1 + (rng.next_f64() * 6.) as u8;
    let magnitude = 1 + (rng.next_f64() * 6.) as u8;
    let p = Placement {
        identity: identity.into(),
        tick: w.tick,
        d10,
        chance,
        placed: d10 as i32 <= chance,
        deviation_d6: [direction, magnitude],
        course_deviation_deg: if d10 as i32 <= chance {
            0
        } else {
            (if direction <= 3 { 1 } else { -1 }) * magnitude as i32 * 5
        },
    };
    state.placements.insert(identity.into(), p.clone());
    Ok(p)
}
pub fn trace(w: &mut World, actor: EntityId, detail: Value) {
    super::aaw2026::trace(w, actor, "6.3.source_fcs", detail)
}
/// Explicit referee fixture adapter for binding only a known own-side success.
pub fn record_own_contact(
    w: &mut World,
    side: Side,
    observer: EntityId,
    sensor: &str,
    target: EntityId,
) -> Result<(), String> {
    if !w.entity(observer).is_some_and(|e| e.side == side) {
        return Err("own observer required".into());
    }
    let observer_position = w.entity(observer).unwrap().pos;
    let contact = w.sides[side.index()]
        .contacts
        .values()
        .find(|c| c.about == target && !c.stale)
        .ok_or("fixture own contact required")?;
    let sensor_kind =
        w.db.sensors
            .get(sensor)
            .ok_or("fixture fitted sensor required")?
            .kind;
    let receiver = w
        .rules
        .detection
        .as_ref()
        .and_then(|s| {
            s.config
                .bindings
                .get(&format!("{}:{sensor}", w.entity(observer).unwrap().class))
        })
        .cloned();
    let report = super::detection2026::SensorReport {
        bearing_alternatives:Vec::new(),
        observer,
        target,
        sensor: sensor.into(),
        at: w.tick,
        observer_position,
        bearing_deg: observer_position.bearing_to(contact.estimate),
        measured_position: (!contact.bearing_only).then_some(contact.estimate),
        bearing_error_deg: None,
        sensor_kind,
        receiver,
    };
    record_detection(w, &report);
    trace(
        w,
        observer,
        json!({"observer_source_track":true,"sensor":sensor}),
    );
    Ok(())
}

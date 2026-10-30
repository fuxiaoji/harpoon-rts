//! Per-side observation projection — the *only* sanctioned way for any client
//! (web UI, LLM staff, RL agent, MCP tool) to read the world. Enemy truth
//! never leaves this module; clients see contacts with estimates.

use crate::events::Event;
use crate::types::{ContactId, DayNight, EntityId, PlatformKind, Side};
use std::collections::BTreeMap;
use crate::world::{VictoryResult, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitObs {
    pub id: EntityId,
    pub class: String,
    pub name: String,
    #[serde(default,skip_serializing_if="is_false")]
    pub manual_control:bool,
    pub kind: PlatformKind,
    pub side: Side,
    pub pos: crate::geom::Vec2,
    pub course_deg: f64,
    pub speed_kts: f64,
    pub ordered_course_deg: f64,
    pub ordered_speed_kts: f64,
    /// Own standing navigation only. Missing in historical recorded frames.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub navigation:Option<NavigationObs>,
    pub depth_m: f64,
    pub altitude_m: f64,
    pub hp: f64,
    pub hp_max: f64,
    /// Own functional casualties only; enemy contacts never expose this map.
    #[serde(default,skip_serializing_if="BTreeMap::is_empty")]
    pub functional_damage:BTreeMap<String,f64>,
    /// Own effective signature damage only; no enemy hit/scan evidence.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub signature_damage:Option<serde_json::Value>,
    /// Own original reports, explicit voice-fit choices and warning receipts.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub communications:Option<serde_json::Value>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub unmanned:Option<serde_json::Value>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub battery_state:Option<serde_json::Value>,
    /// Own physical projector stocks and reload availability; no target truth.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub projectors:Option<serde_json::Value>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub depth_charges:Option<serde_json::Value>,
    /// Only actual own-side measured survivor reports, with opaque labels.
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub survivors:Vec<serde_json::Value>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub nuclear:Option<serde_json::Value>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub expanded_damage:Option<serde_json::Value>,
    /// Own forward-only critical repair ledger and manually selected assistance.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub repairs:Option<serde_json::Value>,
    pub fire_on: bool,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub engagement_policy:Option<crate::commands::LaunchEngagement>,
    pub assigned: Option<ContactId>,
    pub weapons: Vec<WeaponObs>,
    pub sensors: Vec<SensorObs>,
    pub chaff: u16,
    pub decoys: u16,
    pub endurance_s: Option<f64>,
    #[serde(default)]
    pub flight_profile: bool,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub aviation:Option<serde_json::Value>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub detection:Option<serde_json::Value>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub ground:Option<serde_json::Value>,
    /// Own source tube log, servicing clocks and declared torpedo releases.
    /// This is never the underwater projectile's referee state.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub torpedoes:Option<serde_json::Value>,
    /// Own launch plans and inventory only; no referee seeker/dice state.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub antiship:Option<serde_json::Value>,
    #[serde(default)]
    pub offensive_jammer: Option<OffensiveJammerObs>,
    pub deck: Option<DeckObs>,
    pub mission: Option<String>,
    /// Own task intent, with own contact IDs only; no enemy entity references.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub air_task:Option<crate::world::MissionState>,
    pub systems: crate::world::Systems,
    /// 潜艇是否在跃变层之下（>150m，声纳探测 ×0.5）。
    pub below_layer: Option<bool>,
    /// Historical v1-v3 normalized battery value. Unbound v4 source charge
    /// is None, including when a save retains an old normalized ext scalar.
    pub battery: Option<f64>,
    /// 拖曳阵是否展开（航速 >25kt 自动收起）。
    pub towed_deployed: Option<bool>,
}
fn is_false(v:&bool)->bool{!*v}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct NavigationObs {
    pub waypoints:Vec<crate::geom::Vec2>,
    pub station:Option<crate::world::StationState>,
    pub manual_flight:bool,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub source_rules:Option<serde_json::Value>,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct OffensiveJammerObs {pub clutter:i32,pub spot_targets:usize,pub standoff:bool,pub source:String}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeaponObs {
    pub weapon: String,
    pub name: String,
    pub ammo: u16,
    pub cooldown_s: f64,
    #[serde(default="default_operational",skip_serializing_if="is_operational")]
    pub operational:bool,
    #[serde(default,skip_serializing_if="is_zero")]
    pub min_launch_altitude_m:f64,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub source_caliber_mm:Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorObs {
    pub sensor: String,
    pub name: String,
    pub active: bool,
    #[serde(default="default_operational",skip_serializing_if="is_operational")]
    pub operational:bool,
    /// Own verified annex row only. None preserves historical wire bytes;
    /// verified=false is explicit missing data under the v3 profile.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub annex:Option<serde_json::Value>,
    /// Nominal detection range vs air/surface/sub targets in nm (UI rings).
    pub vs_air: Option<f64>,
    pub vs_surface: Option<f64>,
    pub vs_sub: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeckObs {
    pub ready: Vec<(String, u8)>,
    pub cycling: Vec<(String, u8, i64)>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub queued:Vec<QueuedAirObs>,
    #[serde(default="default_operational",skip_serializing_if="is_operational")]
    pub can_launch:bool,
    #[serde(default="default_operational",skip_serializing_if="is_operational")]
    pub can_recover:bool,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct QueuedAirObs {pub class:String,pub launch_at:i64,pub mode:String}
fn default_operational()->bool{true}
fn is_operational(v:&bool)->bool{*v}
fn is_zero(v:&f64)->bool{*v==0.0}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContactObs {
    pub id: ContactId,
    /// None is an unclassified emitter/contact, never the referee's true kind.
    pub category: Option<PlatformKind>,
    pub class_name: Option<String>,
    /// Dead-reckoned estimate at the current tick.
    pub estimate: crate::geom::Vec2,
    pub est_course_deg: f64,
    pub est_speed_kts: f64,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub motion_known:Option<bool>,
    pub quality: f64,
    pub bearing_only: bool,
    pub last_seen_s: f64,
    pub stale: bool,
    pub sensors: Vec<String>,
    /// Shooter-side battle-damage assessment. Only populated from hits this
    /// side actually observed on a tracked contact.
    #[serde(default)]
    pub bda_hits: u32,
    #[serde(default)]
    pub bda_damage: f64,
    #[serde(default)]
    pub bda_hp_fraction: Option<f64>,
    /// Known own ordnance already in flight at this contact. Lets a commander (or
    /// the AI staff) avoid stacking redundant salvos on an already-targeted
    /// contact and keep magazines in reserve.
    /// None means at least one released torpedo has no qualified status report;
    /// its referee-only retirement must not turn an unknown count into zero.
    #[serde(default="known_zero")]
    pub own_in_flight: Option<u32>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub own_torpedo_releases:Option<serde_json::Value>,
}
fn known_zero()->Option<u32>{Some(0)}

/// Objective display for an exercise. It deliberately exposes only the
/// player's own confirmed kills and losses; the authoritative enemy total is
/// revealed in `result` when the scenario ends.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VictoryObservation {
    /// `area_denial`, `kill_count`, or `vp`.
    pub mode: String,
    pub duration_s: i64,
    pub remaining_s: i64,
    pub kill_count_at_end: bool,
    pub target_vp: f64,
    pub own_confirmed_kills: u32,
    pub own_losses: u32,
    /// Confirmed kills per class ("ship"/"sub"/"aircraft"/"base").
    #[serde(default)]
    pub own_kills_by_kind: BTreeMap<String, u32>,
    /// Own losses per class.
    #[serde(default)]
    pub own_losses_by_kind: BTreeMap<String, u32>,
    /// Scenario kill weights, when configured; ships/subs can outweigh
    /// aircraft. Absent means every platform counts 1.
    #[serde(default)]
    pub kill_weights: Option<crate::scenario::KillWeights>,
    /// Own weighted score under those weights (0.0 without weights).
    #[serde(default)]
    pub own_kill_points: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    #[serde(default,skip_serializing_if="crate::rules::damage_runtime::RuleProfile::is_legacy")]
    pub rules_profile:crate::rules::damage_runtime::RuleProfile,
    #[serde(default)]
    pub control_version:u8,
    pub tick: i64,
    pub time_s: f64,
    pub scenario_id: String,
    pub scenario_name: String,
    #[serde(default)]
    pub scenario_description: String,
    pub theater: Option<crate::scenario::Theater>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub radar_environment: Option<crate::rules::electronic_warfare::RadarEnvironment>,
    pub mission: Option<crate::mission::MissionObservation>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub mines:Option<serde_json::Value>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub exercise:Option<crate::exercise::ExerciseView>,
    #[serde(default)]
    pub victory: Option<VictoryObservation>,
    pub automated_side: Option<Side>,
    pub your_side: Side,
    pub over: bool,
    pub result: Option<VictoryResult>,
    pub weather: crate::world::Weather,
    pub day_night: DayNight,
    pub units: Vec<UnitObs>,
    pub contacts: Vec<ContactObs>,
    pub events: Vec<Event>,
}

/// Internal source snapshot for a physically qualified own telemetry link.
/// It deliberately bypasses telemetry filtering; no public endpoint exposes it.
pub(crate) fn own_unit_snapshot(world:&World,id:EntityId)->Option<UnitObs>{
    let e=world.entity(id)?;let side=e.side;
    Some(UnitObs {
            id: e.id,
            class: e.class.clone(),
            name: e.name.clone(),
            manual_control:e.ext.get("control.manual")==Some(&1.0),
            kind: e.kind,
            side: e.side,
            pos: e.pos,
            course_deg: e.course_deg,
            speed_kts: e.speed_kts,
            ordered_course_deg: e.ordered_course_deg,
            ordered_speed_kts: e.ordered_speed_kts,
            navigation:if !e.waypoints.is_empty()||e.station.is_some()||e.ext.get("air.manual_control")==Some(&1.0)||crate::rules::navigation2026::configured(world,e.id)
                || e.kind==PlatformKind::Sub&&crate::rules::submarine_annex::capability(world,&e.class).is_some() {
                Some(NavigationObs{waypoints:e.waypoints.iter().copied().collect(),station:e.station.clone(),manual_flight:e.ext.get("air.manual_control")==Some(&1.0),source_rules:crate::rules::navigation2026::own_view(world,e.id)})
            }else{None},
            depth_m: e.depth_m,
            altitude_m: e.altitude_m,
            hp: e.hp,
            hp_max: crate::rules::beached2026::capacity(world,e.id).unwrap_or(e.hp_max),
            functional_damage:e.ext.iter().filter(|(k,v)|k.starts_with("damage.")&&**v>0.0).map(|(k,v)|(k.clone(),*v)).collect(),
            signature_damage:crate::rules::signature_damage2026::own_view(world,e.id),
            communications:crate::rules::communications2026::own_view(world,e.id),
            unmanned:crate::rules::unmanned2026::own_view(world,e.id),
            battery_state:crate::rules::battery2026::own_view(world,e.id),
            projectors:{let v=crate::rules::asw_weapons2026::projector_own_view(world,e.id);(!v.is_null()).then_some(v)},
            depth_charges:{let v=crate::rules::asw_weapons2026::depth_charge_own_view(world,e.id);(!v.is_null()).then_some(v)},
            survivors:crate::rules::expanded_damage2026::survivor_view(world,e.id),
            nuclear:crate::rules::nuclear2026::own_view(world,e.id),
            expanded_damage:crate::rules::expanded_damage2026::own_view(world,e.id),
            fire_on: e.fire_on,
            engagement_policy: e.ext.get("doctrine.max_in_flight").map(|n|crate::commands::LaunchEngagement {
                air_only:e.ext.get("doctrine.air_only")==Some(&1.0),max_in_flight:*n as u8}),
            assigned: e.assigned,
            weapons: e
                .weapons
                .iter()
                .map(|m| WeaponObs {
                    weapon: m.weapon.clone(),
                    name: world
                        .db
                        .weapons
                        .get(&m.weapon)
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| m.weapon.clone()),
                    ammo: m.ammo,
                    cooldown_s: m.cooldown_s,
                    operational:m.mounts>0&&(!crate::rules::damage_runtime::active(world)||crate::rules::damage_runtime::weapons_operational(e))&&(!crate::rules::ground2026::active(world)||crate::rules::ground2026::operational(world,e.id))&&crate::rules::expanded_damage2026::weapon_operational(world,e.id,&m.weapon).unwrap_or(false),
                    min_launch_altitude_m:if crate::rules::damage_runtime::active(world){world.db.weapons.get(&m.weapon).map(crate::rules::damage_runtime::minimum_release_altitude).unwrap_or(0.0)}else{0.0},
                    source_caliber_mm:if crate::rules::operations::active(world){crate::rules::operations::gun_caliber(&e.class,&m.weapon)}else{None},
                })
                .collect(),
            sensors: e
                .sensors
                .iter()
                .map(|s| {
                    let (mut a, mut sf, sb) =
                        crate::sensors::nominal_ranges(&world.db, e, s);
                    let annex=world.db.sensors.get(&s.sensor).filter(|d|crate::rules::operations::active(world)&&d.kind==crate::db::SensorKind::Radar).map(|d| {
                        if let Some(p)=crate::rules::aviation2026::operations_radar_profile(world,e,d) {
                            a=Some(p["air"].as_array().unwrap().iter().filter_map(|n|n.as_f64()).fold(0.0,f64::max));
                            sf=Some(p["surface"].as_array().unwrap().iter().filter_map(|n|n.as_f64()).fold(0.0,f64::max));
                            let mut row=p.clone();row["verified"]=serde_json::json!(true);row
                        }else{a=Some(0.0);sf=Some(0.0);serde_json::json!({"verified":false})}
                    });
                    SensorObs {
                        sensor: s.sensor.clone(),
                        name: world
                            .db
                            .sensors
                            .get(&s.sensor)
                            .map(|d| d.name.clone())
                            .unwrap_or_else(|| s.sensor.clone()),
                        active: world.db.sensors.get(&s.sensor).map(|d| if d.kind == crate::db::SensorKind::Radar {crate::sensors::radar_emitting(e)} else {s.active}).unwrap_or(s.active),
                        operational:!crate::sensors::sensor_damaged(e,&s.sensor)&&e.ext.get("damage.power_lost").copied().unwrap_or(0.0)==0.0&&crate::rules::expanded_damage2026::sensor_operational(world,e.id,&s.sensor).unwrap_or(false),
                        annex,
                        vs_air: a,
                        vs_surface: sf,
                        vs_sub: sb,
                    }
                })
                .collect(),
            chaff: e.chaff,
            decoys: e.decoys,
            endurance_s: e.endurance_s,
            flight_profile:crate::rules::flight_profiles::supported(&e.class),
            aviation:crate::rules::aviation2026::own_view(world,e.id),
            ground:crate::rules::ground2026::own_view(world,e.id),
            repairs:crate::rules::repairs2026::own_view(world,e.id),
            torpedoes:crate::rules::torpedo2026::own_view(world,side,e.id),
            antiship:crate::rules::surface2026::antiship::own_view(world,e.id),
            detection:crate::rules::detection2026::own_view(world,side,e.id),
            offensive_jammer:crate::rules::electronic_warfare::capability(&e.class).map(|c|OffensiveJammerObs {clutter:c.clutter,spot_targets:c.spot_targets,standoff:c.standoff,source:c.source.clone()}),
            deck: e.deck.as_ref().map(|d| DeckObs {
                ready: d.ready.iter().map(|(c,n)|(c.clone(),n.saturating_sub(crate::rules::operations::reserved(world,e.id,c).min(255) as u8))).collect(),
                cycling: d.cycling.clone(),
                queued:world.rules.operations.as_ref().map(|s|s.launches.iter().filter(|j|j.carrier==e.id).map(|j|QueuedAirObs{class:j.class.clone(),launch_at:j.due,mode:j.mode.clone()}).collect()).unwrap_or_default(),
                can_launch:crate::rules::expanded_damage2026::bridge_flight_operational(world,e.id)&&(!crate::rules::damage_runtime::active(world)||crate::rules::damage_runtime::flight_operational(e,true,world.tick))&&crate::rules::expanded_damage2026::flight_gear_operational(world,e.id,true).unwrap_or(false),
                can_recover:crate::rules::expanded_damage2026::bridge_flight_operational(world,e.id)&&(!crate::rules::damage_runtime::active(world)||crate::rules::damage_runtime::flight_operational(e,false,world.tick))&&crate::rules::expanded_damage2026::flight_gear_operational(world,e.id,false).unwrap_or(false),
            }),
            mission: e.mission.as_ref().map(|m| m.tag().to_string()),
            air_task:e.mission.clone(),
            systems: e.systems.clone(),
            below_layer: (e.kind == PlatformKind::Sub).then(|| e.depth_m > 150.0),
            battery: if world.rules.profile==crate::rules::damage_runtime::RuleProfile::Rts2026Full {
                None
            } else { e.ext.get("battery").copied() },
            towed_deployed: e
                .ext
                .get("tow_deployed")
                .map(|v| *v > 0.5)
                .filter(|_| e.kind == PlatformKind::Ship),
        })
}

/// An authored own roster is enough to request a radio link or control change;
/// it never authorizes weapon fire or supplies missing vehicle telemetry.
pub fn known_unmanned_unit(obs:&Observation,id:EntityId)->bool{
    obs.units.iter().any(|u|u.id==id||u.unmanned.as_ref().and_then(|v|v["controlled"].as_array()).is_some_and(|rows|rows.iter().any(|r|r["unit"].as_u64()==Some(id.0 as u64))))
}
/// Project the world as seen by `side`. `since_seq` filters the event tail.
pub fn observe(world: &World, side: Side, since_seq: u64, max_events: usize) -> Observation {
    let mut units:Vec<UnitObs> = world
        .entities
        .values()
        .filter(|e| e.side == side && e.alive && !(crate::rules::torpedo2026::active(world) && e.kind==PlatformKind::Torpedo))
        .filter_map(|e| {
            if crate::rules::unmanned2026::is_configured(world,e.id) {
                let t=crate::rules::unmanned2026::telemetry(world,e.id)?;
                let mut unit=t.unit?;unit.pos=t.position;
                if let Some(view)=unit.unmanned.as_mut(){view["telemetry"]=serde_json::json!({"position_at":t.position_at,"status_at":t.status_at});}
                Some(unit)
            } else {own_unit_snapshot(world,e.id)}
        })
        .collect();

    for id in crate::rules::unmanned2026::known_units(world,side) {
        if units.iter().any(|u|u.id==id){continue}
        if let Some(t)=crate::rules::unmanned2026::telemetry(world,id) {
            if let Some(mut unit)=t.unit {unit.pos=t.position;
                if let Some(view)=unit.unmanned.as_mut(){view["telemetry"]=serde_json::json!({"position_at":t.position_at,"status_at":t.status_at});}
                units.push(unit);}
        }
    }
    units.sort_by_key(|u|u.id);

    let mut own_in_flight: std::collections::BTreeMap<EntityId, u32> = Default::default();
    for e in world.entities.values() {
        if !e.alive || !e.kind.is_projectile() {
            continue;
        }
        // A launched underwater weapon is a declaration, not continuous truth
        // telemetry. Its public release log below survives secret retirement.
        if crate::rules::torpedo2026::active(world) && e.kind==PlatformKind::Torpedo {continue;}
        if let Some(p) = &e.projectile {
            if p.launched_by_side == side {
                if let Some(t) = p.target {
                    *own_in_flight.entry(t).or_insert(0) += 1;
                }
            }
        }
    }

    let contacts = world.sides[side.index()]
        .contacts
        .values()
        .map(|c| {let torpedoes=crate::rules::torpedo2026::unreported_releases(world,side,c.id); ContactObs {
            id: c.id,
            category: if world.rules.profile==crate::rules::damage_runtime::RuleProfile::Rts2026Full{crate::rules::detection2026::known_category(world,side,c.about)}else{Some(c.category)},
            class_name: c.class_name.clone(),
            estimate: c.estimate_at(world.tick),
            est_course_deg: c.est_course_deg,
            est_speed_kts: c.est_speed_kts,
            motion_known:crate::rules::operations::motion_known(world,side,c.about),
            quality: c.quality,
            bearing_only: c.bearing_only,
            last_seen_s: (world.tick - c.last_update_tick) as f64 * crate::world::TICK_SECONDS,
            stale: c.stale,
            sensors: c.sensors.clone(),
            bda_hits: world.sides[side.index()].bda.get(&c.about).map(|b| b.hits).unwrap_or(0),
            bda_damage: world.sides[side.index()].bda.get(&c.about).map(|b| b.damage).unwrap_or(0.0),
            bda_hp_fraction: world.sides[side.index()].bda.get(&c.about).and_then(|b| b.hp_fraction),
            own_in_flight: if torpedoes.is_some(){None}else{Some(own_in_flight.get(&c.about).copied().unwrap_or(0))},
            own_torpedo_releases:torpedoes,
        }})
        .collect();

    let mut events = world.events_for(side, since_seq);
    if events.len() > max_events {
        events = events.split_off(events.len() - max_events);
    }
    let events = events.into_iter().cloned().collect();
    let (concealed_loss_count,concealed_loss_kinds)=if world.over{Default::default()}else{crate::rules::unmanned2026::concealed_losses(world,side)};

    Observation {
        rules_profile:world.rules.profile,
        control_version:3,
        tick: world.tick,
        time_s: world.time_s(),
        scenario_id: world.scenario_id.clone(),
        scenario_name: world.scenario_name.clone(),
        scenario_description: world.scenario_description.clone(),
        theater: world.context.theater.clone(),
        radar_environment: world.context.radar_environment.clone(),
        mission: crate::mission::observe(world, side),
        mines:crate::rules::mines2026::own_view(world,side),
        exercise:crate::exercise::observe(world,side),
        victory: Some(VictoryObservation {
            mode: if world.context.island_exercise.is_some() {"exercise".into()}else if world.context.area_denial.is_some() {
                "area_denial".into()
            } else if world.scenario_kill_count_at_end {
                "kill_count".into()
            } else {
                "vp".into()
            },
            duration_s: world.duration_s,
            remaining_s: (world.duration_s - world.tick).max(0),
            kill_count_at_end: world.scenario_kill_count_at_end,
            target_vp: world.scenario_target_vp,
            own_confirmed_kills: world.sides[side.index()].confirmed_kills,
            own_losses: world.losses[side.index()].saturating_sub(concealed_loss_count),
            own_kills_by_kind: world.sides[side.index()].confirmed_kills_by_kind.clone(),
            own_losses_by_kind: world.losses_by_kind.iter()
                .filter(|(kind,n)|world.rules.profile!=crate::rules::damage_runtime::RuleProfile::Rts2026Full || n[side.index()]>concealed_loss_kinds.get(*kind).copied().unwrap_or(0))
                .map(|(kind, n)| (kind.clone(), n[side.index()].saturating_sub(concealed_loss_kinds.get(kind).copied().unwrap_or(0))))
                .collect(),
            kill_weights: world.scenario_kill_weights.clone(),
            own_kill_points: world.kill_score(side),
        }),
        automated_side: world.context.automated_side,
        your_side: side,
        over: world.over,
        result: world.result.clone(),
        weather: world.weather.clone(),
        day_night: world.day_night,
        units,
        contacts,
        events,
    }
}

//! English2026 8.3: explicit source gun service, atomic Tactical-Turn attack.
//! Own native visual admission precedes referee aspect/armor and original die.
use crate::db::WeaponKind;
use crate::rules::{damage_runtime as dr, ground2026::ngfs::GunneryStandard};
use crate::types::PlatformKind;
use crate::{ContactId, EntityId, EventKind, Rng, Side, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};
#[path = "gun_annex_extra.rs"]
mod gun_annex_extra;
pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        let mut data: Value = serde_json::from_str(include_str!(
            "../../../../../data/rules/surface-gunnery-2026.json"
        )).unwrap();
        gun_annex_extra::merge(&mut data);
        data
    })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit {
    pub source: String,
    pub weapon: String,
    pub standard: GunneryStandard,
    pub barrels: u8,
    pub arcs: Vec<[f64; 2]>,
    pub under_fire: bool,
    pub local_control: bool,
    pub radar_gfc: bool,
    pub reaction_only: bool,
    pub terrain_clear: bool,
    pub other_conditions_clear: bool,
    #[serde(default)]
    pub ammunition_per_attack: Option<u16>,
    /// Separate author-declared starshell rounds for this physical mount.
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub starshell_stock: Option<u16>,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub starshell_per_turn: Option<u16>,
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub starshell_mount_role: Option<String>,
    /// Author-assigned native physical index, not a real-world serial number.
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub starshell_physical_index: Option<usize>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub units: BTreeMap<String, Fit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub actor: EntityId,
    pub target: EntityId,
    pub contact: ContactId,
    pub tick: i64,
    pub weapon: String,
    #[serde(default, skip_serializing_if="is_false")]
    pub starshell: bool,
    #[serde(default, skip_serializing_if="is_zero")]
    pub physical_index:usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Illumination { pub actor:EntityId, pub target:EntityId, #[serde(default)] pub weapon:String, #[serde(default)] pub physical_index:usize, pub turn:i64, pub effect:crate::rules::detection2026::StarshellEffect,
    #[serde(default,skip_serializing_if="Option::is_none")] pub light_position:Option<crate::Vec2> }
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub fits: BTreeMap<EntityId, Fit>,
    #[serde(default,skip_serializing_if="BTreeMap::is_empty")]
    pub illumination:BTreeMap<String, Illumination>,
    pub jobs: BTreeMap<String, Job>,
    pub last_fire: BTreeMap<String, i64>,
    /// Referee evidence exists only from a native source-gun hit until the
    /// common damage consumer takes it. No player command supplies this.
    pub impacts: BTreeMap<String, GunImpact>,
    #[serde(
        default,
        skip_serializing_if = "crate::rules::expanded_damage2026::gun_deck::State::is_empty"
    )]
    pub gun_deck: crate::rules::expanded_damage2026::gun_deck::State,
}
impl State {
    pub fn is_empty(&self) -> bool {
        self.fits.is_empty()
            && self.illumination.is_empty()
            && self.jobs.is_empty()
            && self.last_fire.is_empty()
            && self.impacts.is_empty()
            && self.gun_deck.is_empty()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GunImpact {
    pub actor: EntityId,
    pub tactical_turn: i64,
    pub range_band: usize,
    pub caliber_mm: f64,
}
pub(crate) fn take_impact(w: &mut World, target: EntityId, identity: &str) -> Option<GunImpact> {
    w.rules
        .aaw
        .as_mut()?
        .surface
        .gunnery
        .impacts
        .remove(&format!("{}:{identity}", target.0))
}
fn is_false(value:&bool)->bool{!*value}
fn is_zero(value:&usize)->bool{*value==0}
fn tac(t: i64) -> i64 {
    t.div_euclid(
        tables()["execution"]["tactical_turn_seconds"]
            .as_i64()
            .unwrap(),
    )
}
fn state(w: &World) -> Option<&State> {
    Some(&w.rules.aaw.as_ref()?.surface.gunnery)
}
fn slot(actor: EntityId, weapon: &str) -> String { physical_slot(actor,weapon,0) }
fn physical_slot(actor:EntityId,weapon:&str,physical:usize)->String {
    if physical==0 {format!("{}:{weapon}",actor.0)} else {format!("{}:{weapon}:physical:{physical}",actor.0)}
}
pub(crate) fn committed_physical(w:&World,actor:EntityId,weapon:&str,physical:usize)->bool {
    state(w).is_some_and(|s|s.last_fire.get(&physical_slot(actor,weapon,physical))==Some(&tac(w.tick)))
}
pub(crate) fn committed(w: &World, actor: EntityId, weapon: &str) -> bool {
    state(w).is_some_and(|s| s.last_fire.get(&slot(actor, weapon)) == Some(&tac(w.tick)))
}
/// The existing RTS one-ship-gun slot and conflict policy is common to both
/// native surface and NGFS consumers. No provider order chooses a free shot.
pub(crate) fn arbitrate_domains(w: &mut World) {
    let land: BTreeSet<_> = w.rules.ground.as_ref().and_then(|s|s.ngfs.as_ref())
        .map(|s|s.jobs.iter().filter(|j|tac(j.declared_tick)==tac(w.tick))
            .map(|j|(j.ship,j.gun.clone(),j.physical_index)).collect()).unwrap_or_default();
    let mut conflicts: BTreeSet<_> = state(w).map(|s|s.jobs.values()
        .filter(|j|tac(j.tick)==tac(w.tick) && land.contains(&(j.actor,j.weapon.clone(),j.physical_index)))
        .map(|j|(j.actor,j.weapon.clone(),j.physical_index)).collect()).unwrap_or_default();
    let local: BTreeSet<_> = state(w).map(|s| {
        let mut groups:BTreeMap<(EntityId,String,usize),BTreeSet<(EntityId,bool)>>=BTreeMap::new();
        for j in s.jobs.values().filter(|j|tac(j.tick)==tac(w.tick)) {
            groups.entry((j.actor,j.weapon.clone(),j.physical_index)).or_default().insert((j.target,j.starshell));
        }
        groups.into_iter().filter(|(_,v)|v.len()>1).map(|(k,_)|k).collect()
    }).unwrap_or_default();
    if let Some(g)=w.rules.aaw.as_mut().map(|a|&mut a.surface.gunnery) {g.jobs.retain(|_,j|!local.contains(&(j.actor,j.weapon.clone(),j.physical_index)));}
    for (actor,weapon,physical) in local {trace(w,actor,"8.3.intent_conflict",json!({"weapon":weapon,"physical_index":physical,"reason":"conflicting declarations for one physical mount; no stock or slot spent"}));}
    if conflicts.is_empty() {return}
    if let Some(s)=w.rules.ground.as_mut().and_then(|s|s.ngfs.as_mut()) {
        s.jobs.retain(|j|!conflicts.contains(&(j.ship,j.gun.clone(),j.physical_index)));
    }
    if let Some(s)=w.rules.aaw.as_mut().map(|a|&mut a.surface.gunnery) {
        s.jobs.retain(|_,j|!conflicts.contains(&(j.actor,j.weapon.clone(),j.physical_index)));
    }
    for (actor,weapon,physical) in conflicts {
        trace(w,actor,"8.3.intent_conflict",json!({"weapon":weapon,"physical_index":physical,"reason":"conflicting surface/NGFS declarations for the same physical ship-gun; no slot or ammunition spent","policy":"existing RTS atomic intent arbitration"}));
    }
}
fn source_hull(e: &crate::world::Entity) -> bool {
    dr::platform_of(e).is_some_and(|p| {
        !p.source.trim().is_empty()
            && matches!(p.size.as_str(), "A" | "B" | "C" | "D" | "E" | "F" | "G")
            && p.belt_cm.is_finite()
            && p.belt_cm >= 0.
            && p.deck_cm.is_finite()
            && p.deck_cm >= 0.
    })
}
fn trace(w: &mut World, actor: EntityId, rule: &str, v: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "surface_gunnery2026".into(),
            rule: rule.into(),
            side: w.entity(actor).map(|e| e.side),
            unit: Some(actor),
            contact: None,
            detail: v.to_string(),
        },
        Visibility::Referee,
    );
}
fn named(w: &World, name: &str) -> Result<EntityId, String> {
    let v: Vec<_> = w.entities.values().filter(|e| e.name == name).collect();
    if v.len() == 1 {
        Ok(v[0].id)
    } else {
        Err("8.3 service requires one exact named firing body".into())
    }
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    let Some(c) = w.context.surface_gunnery_2026.clone() else {
        return Ok(());
    };
    if !c.source.starts_with("scenario_assumption:") {
        return Err("8.3 experimental fittings require an explicit author source".into());
    }
    let mut s = State::default();
    for (name, f) in c.units {
        let id = named(w, &name)?;
        let e = w.entity(id).unwrap();
        if e.kind != PlatformKind::Ship
            || !f.source.starts_with("scenario_assumption:")
            || f.barrels == 0
            || f.barrels > 12
            || f.arcs.is_empty()
            || !f.arcs.iter().all(|a| {
                a.iter().all(|n| n.is_finite()) && a[0] >= 0. && a[1] <= 360. && a[0] <= a[1]
            })
        {
            return Err("8.3 invalid author firing ship/barrels/arcs".into());
        }
        if !e
            .weapons
            .iter()
            .any(|m| m.weapon == f.weapon && m.mounts > 0)
            || !w
                .db
                .weapons
                .get(&f.weapon)
                .is_some_and(|d| d.kind == WeaponKind::Gun)
            || tables()["guns"].get(&f.weapon).is_none()
        {
            return Err("8.3 actual fitted gun and verified Annex C card required".into());
        }
        if f.ammunition_per_attack == Some(0) {
            return Err("8.3 declared tracked expenditure must be positive".into());
        }
        if f.standard == GunneryStandard::Manual
            && tables()["guns"][&f.weapon]["caliber_mm"].as_f64().unwrap()
                > tables()["manual"]["maximum_caliber_mm"].as_f64().unwrap()
        {
            return Err("8.3 manual standard is restricted to caliber <=60mm".into());
        }
        s.fits.insert(id, f);
    }
    w.rules
        .aaw
        .as_mut()
        .ok_or("8.3 source surface ledger unavailable")?
        .surface
        .gunnery = s;
    Ok(())
}
pub fn configured(w: &World, actor: EntityId, weapon: &str) -> bool {
    state(w).is_some_and(|s| s.fits.get(&actor).is_some_and(|f| f.weapon == weapon))
}
fn native_visual(w: &World, actor: EntityId, cid: ContactId) -> bool {
    crate::rules::communications2026::own_view(w, actor)
        .and_then(|v| v["reports"].as_array().cloned())
        .is_some_and(|reports| {
            reports.iter().any(|r| {
                r["contact"] == json!(cid)
                    && r["sensor"] == "visual"
                    && r["bearing_only"] == false
                    && r["observed_at"].as_i64().is_some_and(|at| {
                        at <= w.tick
                            && w.tick - at
                                <= tables()["execution"]["native_report_maximum_age_s"]
                                    .as_i64()
                                    .unwrap()
                    })
            })
        })
}
fn native_visual_or_radar(w:&World,actor:EntityId,cid:ContactId)->bool {
    if native_visual(w,actor,cid) {return true;}
    let Some(e)=w.entity(actor) else{return false};
    crate::rules::communications2026::own_view(w,actor).and_then(|v|v["reports"].as_array().cloned()).is_some_and(|reports|reports.iter().any(|r| {
        r["contact"]==json!(cid) && r["bearing_only"]==false && r["observed_at"].as_i64().is_some_and(|at|at<=w.tick && w.tick-at<=tables()["execution"]["native_report_maximum_age_s"].as_i64().unwrap()) && r["sensor"].as_str().is_some_and(|sensor|e.sensors.iter().any(|s|s.sensor==sensor) && w.db.sensors.get(sensor).is_some_and(|d|d.kind==crate::db::SensorKind::Radar))
    }))
}
fn band(w: &str, range: f64) -> Result<usize, String> {
    tables()["guns"][w]["bands"]
        .as_array()
        .unwrap()
        .iter()
        .position(|b| range <= b["max_range_kyd"].as_f64().unwrap() / 2.)
        .ok_or("8.3 beyond the verified gun's Extreme band".into())
}
fn validate_impl(
    allow_radar:bool,
    physical:usize,
    w: &World,
    side: Side,
    actor: EntityId,
    cid: ContactId,
    weapon: &str,
    rounds: u8,
) -> Result<EntityId, String> {
    if rounds != 1 {
        return Err("8.3 declare one gun attack, not individual shell dice".into());
    }
    let f = state(w)
        .and_then(|s| s.fits.get(&actor))
        .filter(|f| f.weapon == weapon)
        .ok_or("8.3 gun service not source qualified; legacy range-fraction Pk forbidden")?;
    let own = w
        .entity(actor)
        .filter(|e| e.alive && e.side == side)
        .ok_or("8.3 active own firing ship required")?;
    if !source_hull(own) {
        return Err("8.3 firing ship source size and armor not qualified".into());
    }
    let env = w
        .rules
        .detection
        .as_ref()
        .and_then(|s| s.config.environment.as_ref())
        .ok_or("8.3 explicit environment required")?;
    let sea = env.sea_state;
    if sea
        > tables()["modifiers"]["no_fire_above_sea_state"]
            .as_u64()
            .unwrap() as usize
    {
        return Err("8.3 sea state prohibits fire".into());
    }
    if sea >= 3 {
        let size = dr::size(own);
        let column = match size {
            "A" | "B" => 0,
            "C" | "D" => 1,
            "E" | "F" | "G" => 2,
            _ => return Err("8.3 firing size unknown for sea-state column".into()),
        };
        if tables()["sea_modifiers"][sea.to_string()][column].is_null() {
            return Err("8.3 source sea-state NFP prohibits fire".into());
        }
    }
    let c = w
        .contact(side, cid)
        .filter(|c| !c.stale)
        .ok_or("8.3 current own target contact required")?;
    if crate::rules::detection2026::known_category(w, side, c.about) != Some(PlatformKind::Ship)
        || !(if allow_radar {native_visual_or_radar(w,actor,cid)} else {native_visual(w, actor, cid)})
    {
        return Err(
            "8.3 requires the firing ship's own native visual solution, not fusion position".into(),
        );
    }
    if !crate::rules::expanded_damage2026::weapon_operational(w, actor, weapon)? {
        return Err("8.3 actual physical gun/director unavailable".into());
    }
    let index=own.weapons.iter().position(|m|m.weapon==weapon).ok_or("8.3 physical mount absent")?;
    if !crate::rules::expanded_damage2026::physical_weapon_mounts(w,actor,index)?.contains(&physical) {return Err("8.3 selected physical mount unavailable".into());}
    if committed_physical(w, actor, weapon, physical)
        || crate::rules::ground2026::ngfs::committed_physical(w, actor, weapon,physical) {
        return Err("8.3 this ship-gun already committed its Tactical Turn attack".into());
    }
    let m = own
        .weapons
        .iter()
        .find(|m| m.weapon == weapon && m.mounts > 0)
        .ok_or("8.3 actual gun mount unavailable")?;
    if f.ammunition_per_attack.is_some_and(|n| m.ammo < n) {
        return Err("8.3 declared shells exceed the actual magazine".into());
    }
    // A target estimate is sufficient for own arc/range and friendly-fire gates.
    let target = c.estimate_at(w.tick);
    let range = own.pos.range_to(target);
    band(weapon, range)?;
    let bearing = own.pos.bearing_to(target);
    let relative = crate::geom::normalize_deg(bearing - own.course_deg);
    if !f.arcs.iter().any(|a| a[0] <= relative && relative <= a[1]) {
        return Err("2.2.1/8.3 target outside explicitly fitted gun arcs".into());
    }
    let medium = tables()["guns"][weapon]["bands"][1]["max_range_kyd"]
        .as_f64()
        .unwrap()
        / 2.;
    if w.entities.values().any(|e| {
        e.id != actor
            && e.alive
            && e.side == side
            && e.kind == PlatformKind::Ship
            && own.pos.range_to(e.pos) <= medium
            && crate::geom::angle_diff(bearing, own.pos.bearing_to(e.pos)).abs()
                <= tables()["line_of_fire"]
                    ["friendly_ship_blocks_if_short_or_medium_and_within_deg"]
                    .as_f64()
                    .unwrap()
    }) {
        return Err("8.3.3 friendly ship blocks the line of fire".into());
    }
    if !f.terrain_clear
        || !f.other_conditions_clear
        || w.context
            .terrain
            .as_ref()
            .is_some_and(|t| t.blocked(own.pos, target))
    {
        return Err(
            "8.3 terrain/obscurant/fire-control geometry unqualified; adjudication pending".into(),
        );
    }
    Ok(c.about)
}
pub fn validate(w:&World,side:Side,actor:EntityId,cid:ContactId,weapon:&str,rounds:u8)->Result<EntityId,String>{
    validate_impl(false,0,w,side,actor,cid,weapon,rounds)
}
pub fn prepare(
    w: &mut World,
    side: Side,
    actor: EntityId,
    cid: ContactId,
    weapon: &str,
    rounds: u8,
) -> Result<(), String> {
    let target = validate(w, side, actor, cid, weapon, rounds)?;
    let key = format!("{}:{}:{weapon}:{}", w.tick, actor.0, target.0);
    w.rules
        .aaw
        .as_mut()
        .unwrap()
        .surface
        .gunnery
        .jobs
        .entry(key)
        .or_insert(Job {
            actor,
            target,
            contact: cid,
            tick: w.tick,
            weapon: weapon.into(),
            starshell: false,
            physical_index:0,
        });
    Ok(())
}
fn starshell_physical(w:&World,actor:EntityId,weapon:&str,fit:&Fit)->Result<usize,String>{
    let e=w.entity(actor).ok_or("5.8.8 mount owner absent")?;
    let mount_index=e.weapons.iter().position(|m|m.weapon==weapon).ok_or("5.8.8 mount absent")?;
    let live=crate::rules::expanded_damage2026::physical_weapon_mounts(w,actor,mount_index)?;
    let selected=match fit.starshell_physical_index {Some(i)=>i,None if live.len()==1=>live[0],None=>return Err("5.8.8 author physical mount index required".into())};
    if !live.contains(&selected){return Err("5.8.8 selected physical mount unavailable".into());} Ok(selected)
}
pub fn validate_starshell(w:&World, side:Side, actor:EntityId, cid:ContactId, weapon:&str)->Result<(),String>{
    let f=state(w).and_then(|s|s.fits.get(&actor)).ok_or("5.8.8 fitted gun required")?;
    let physical=starshell_physical(w,actor,weapon,f)?;
    validate_impl(true,physical,w,side,actor,cid,weapon,1)?;
    let amount=f.starshell_per_turn.filter(|n|*n>0).ok_or("5.8.8 explicit starshell expenditure required")?;
    if f.starshell_stock.unwrap_or(0)<amount {return Err("5.8.8 separate starshell inventory unavailable".into());}
    let e=w.entity(actor).ok_or("5.8.8 actor absent")?;
    let role=f.starshell_mount_role.as_deref().ok_or("5.8.8 author-declared mount role required")?;
    let eligible=match dr::size(e) {"A"|"B"=>role=="secondary"||role=="special", "C"|"D"|"E"=>role=="primary"||role=="special", _=>role=="special"};
    if !eligible {return Err("5.8.8 mount role not eligible for firing ship size".into());}
    let c=w.contact(side,cid).ok_or("5.8.8 contact absent")?;
    let range=e.pos.range_to(c.estimate_at(w.tick));
    let long=tables()["guns"][weapon]["bands"][2]["max_range_kyd"].as_f64().ok_or("5.8.8 source Long band missing")?/2.;
    let mm=tables()["guns"][weapon]["caliber_mm"].as_f64().ok_or("5.8.8 caliber missing")?;
    crate::rules::detection2026::starshell_effect(mm,range*2.,true,1)?;
    if range>long {return Err("5.8.8 beyond Long range".into());} Ok(())
}
pub fn prepare_starshell(w:&mut World,side:Side,actor:EntityId,cid:ContactId,weapon:&str)->Result<(),String>{
    validate_starshell(w,side,actor,cid,weapon)?;
    let target=w.contact(side,cid).ok_or("5.8.8 contact absent")?.about;
    let key=format!("starshell:{}:{}:{weapon}:{}",w.tick,actor.0,target.0);
    let physical_index=starshell_physical(w,actor,weapon,state(w).unwrap().fits.get(&actor).unwrap())?;
    w.rules.aaw.as_mut().unwrap().surface.gunnery.jobs.entry(key).or_insert(Job{actor,target,contact:cid,tick:w.tick,weapon:weapon.into(),starshell:true,physical_index}); Ok(())
}
// Declared RTS mapping: a committed series exists while its exact source gun
// remains operational within that Tactical Turn. It cannot restart after a
// broken episode without a new declaration.
fn live_episode(w:&World,light:&Illumination)->bool {
    light.turn==tac(w.tick) && w.entity(light.actor).is_some_and(|e|e.alive)
        && !light.weapon.is_empty()
        && state(w).and_then(|s|s.fits.get(&light.actor)).is_some_and(|f|starshell_physical(w,light.actor,&light.weapon,f).ok()==Some(light.physical_index))
}
pub fn interrupt_starshells(w:&mut World) {
    let ended:Vec<String>=state(w).map(|s|s.illumination.iter().filter(|(_,v)|!live_episode(w,v)).map(|(k,_)|k.clone()).collect()).unwrap_or_default();
    if let Some(s)=w.rules.aaw.as_mut().map(|a|&mut a.surface.gunnery) {for key in ended {s.illumination.remove(&key);}}
}
/// Anonymous visible light only; never a contact, target identity or FCS.
pub fn visible_lights(w:&World, observer:EntityId)->Vec<Value> {
    let Some(o)=w.entity(observer).filter(|e|e.alive && e.kind==PlatformKind::Ship) else{return vec![]};
    if w.day_night!=crate::types::DayNight::Night || !w.rules.detection.as_ref().and_then(|s|s.config.environment.as_ref()).is_some_and(|e|e.precipitation==0) {return vec![];}
    state(w).into_iter().flat_map(|s|s.illumination.values()).filter(|light|live_episode(w,light) && light.effect.works && light.light_position.is_some_and(|p|o.pos.range_to(p)<=30.)).map(|_|json!({"kind":"starshell_light"})).collect()
}
pub fn starshell_blinds(w:&World, observer:EntityId,target:EntityId)->bool {
    state(w).is_some_and(|s|s.illumination.values().any(|v|v.actor==observer && v.target==target && live_episode(w,v) && v.effect.blinds_firer))
}
/// Current-turn source illumination; expiry follows the turn boundary.
pub fn illumination(w:&World,observer:EntityId,target:EntityId)->Option<bool>{
    let env=w.rules.detection.as_ref()?.config.environment.as_ref()?;
    illumination_with_environment(w,observer,target,env)
}
pub(crate) fn illumination_with_environment(w:&World,observer:EntityId,target:EntityId,env:&crate::rules::detection2026::Environment)->Option<bool>{
    let observer_body=w.entity(observer)?; let target_body=w.entity(target)?;
    if w.day_night != crate::types::DayNight::Night || env.precipitation != 0 || env.visibility_percent < 10 {return None;}
    let size=match dr::size(target_body) {"A"=>0,"B"=>1,"C"=>2,"D"=>3,"E"=>4,"F"=>5,"G"=>6,_=>return None};
    for light in state(w)?.illumination.values() {
        let firer=w.entity(light.actor)?;
        // The book establishes the firing vessel's benefit; nearby other
        // vessels require geometry qualification that is not yet printed.
        if light.actor!=observer || !live_episode(w,light) || light.target!=target || firer.side!=observer_body.side {continue;}
        // A hit illuminates a physical circle, not a moving truth target.
        if light.effect.illuminated && !light.light_position.is_some_and(|p|p.range_to(target_body.pos)<=light.effect.radius_yards/2025.3718285) {continue;}
        let silhouette=light.effect.silhouetted;
        if !light.effect.illuminated && !silhouette {continue;}
        let max=crate::rules::detection2026::starshell_detection_range(size,silhouette).ok()?;
        if firer.pos.range_to(target_body.pos)*2. <= max {return Some(silhouette);}
    }
    None
}
fn standard_key(v: GunneryStandard) -> &'static str {
    match v {
        GunneryStandard::Manual => "Manual",
        GunneryStandard::Gs4 => "GS4",
        GunneryStandard::Gs5 => "GS5",
        GunneryStandard::Gs6 => "GS6",
    }
}
fn shell_group(mm: f64) -> u8 {
    if mm
        <= tables()["overconcentration_shell_groups_mm"]["small_max"]
            .as_f64()
            .unwrap()
    {
        0
    } else if mm
        <= tables()["overconcentration_shell_groups_mm"]["medium_max"]
            .as_f64()
            .unwrap()
    {
        1
    } else {
        2
    }
}
pub fn resolve(w: &mut World) { resolve_phase(w,false); }
pub fn resolve_starshells(w: &mut World) { resolve_phase(w,true); }
fn resolve_phase(w: &mut World, starshell_phase:bool) {
    if !crate::rules::aaw2026::active(w) {
        return;
    }
    let Some(s) = w.rules.aaw.as_mut().map(|a| &mut a.surface.gunnery) else {
        return;
    };
    let all = std::mem::take(&mut s.jobs);
    let (jobs,remaining):(BTreeMap<_,_>,BTreeMap<_,_>)=all.into_iter().partition(|(_,j)|j.starshell==starshell_phase);
    s.jobs=remaining;
    let mut alternatives: BTreeMap<String, Vec<Job>> = BTreeMap::new();
    for j in jobs.into_values() {
        alternatives
            .entry(physical_slot(j.actor, &j.weapon,j.physical_index))
            .or_default()
            .push(j);
    }
    let jobs: Vec<_> = alternatives
        .into_values()
        .filter_map(|v| {
            if v.len() == 1 {
                v.into_iter().next()
            } else {
                for j in v {
                    trace(
                        w,
                        j.actor,
                        "8.3.intent_conflict",
                        json!({"reason":"conflicting targets for one ship-gun; no expenditure"}),
                    );
                }
                None
            }
        })
        .collect();
    let mut prepared = Vec::new();
    for j in jobs {
        let Some(e) = w.entity(j.actor).cloned() else {
            continue;
        };
        if (if j.starshell {validate_starshell(w,e.side,j.actor,j.contact,&j.weapon)} else {validate(w,e.side,j.actor,j.contact,&j.weapon,1).map(|_|())}).is_err() {
            continue;
        }
        let Some(t) = w
            .entity(j.target)
            .filter(|t| t.alive && t.kind == PlatformKind::Ship)
            .cloned()
        else {
            continue;
        };
        let f = state(w).unwrap().fits[&j.actor].clone();
        let range = e.pos.range_to(t.pos);
        let Ok(b) = band(&j.weapon, range) else {
            continue;
        };
        if f.barrels > 10 {
            trace(
                w,
                j.actor,
                "8.3.pending",
                json!({"reason":"11-12 barrel hit modifier unprinted; no guessed result"}),
            );
            continue;
        }
        let Some(env) = w
            .rules
            .detection
            .as_ref()
            .and_then(|d| d.config.environment.as_ref())
        else {
            continue;
        };
        if env.sea_state > 6 {
            trace(
                w,
                j.actor,
                "8.3.no_fire",
                json!({"reason":"sea state above 6"}),
            );
            continue;
        }
        if !source_hull(&e) || !source_hull(&t) {
            trace(
                w,
                j.actor,
                "8.3.pending",
                json!({"reason":"source ship size/armor not qualified"}),
            );
            continue;
        }
        let need = if j.starshell { 0 } else { f.ammunition_per_attack.unwrap_or(0) };
        if j.starshell { let fit=w.rules.aaw.as_mut().unwrap().surface.gunnery.fits.get_mut(&j.actor).unwrap(); *fit.starshell_stock.as_mut().unwrap() -= f.starshell_per_turn.unwrap(); }
        if let Some(m) = w
            .entity_mut(j.actor)
            .and_then(|e| e.weapons.iter_mut().find(|m| m.weapon == j.weapon))
        {
            m.ammo -= need;
        }
        crate::rules::expanded_damage2026::record_weapon_expenditure(w, j.actor, &j.weapon, need);
        w.rules
            .aaw
            .as_mut()
            .unwrap()
            .surface
            .gunnery
            .last_fire
            .insert(physical_slot(j.actor, &j.weapon,j.physical_index), tac(w.tick));
        prepared.push((j, f, e, t, range, b));
    }
    let mut groups: BTreeMap<(EntityId, u8), BTreeSet<EntityId>> = BTreeMap::new();
    for (j, _, _, _, _, _) in &prepared {
        let mm = tables()["guns"][&j.weapon]["caliber_mm"].as_f64().unwrap();
        groups
            .entry((j.target, shell_group(mm)))
            .or_default()
            .insert(j.actor);
    }
    // RTS phase mapping: illumination resolves before other qualified gun shots.
    prepared.sort_by_key(|(job,_,_,_,_,_)| !job.starshell);
    for (j, f, e, t, range, b) in prepared {
        let env = w
            .rules
            .detection
            .as_ref()
            .unwrap()
            .config
            .environment
            .as_ref()
            .unwrap();
        let vis = env.visibility_percent as f64;
        let lit=illumination(w,j.actor,j.target).is_some();
        let modifiers = &tables()["modifiers"];
        let mut shifts = ((f.barrels - 1) / 2) as f64;
        if lit { shifts += modifiers["illuminated_by_starshell_fire_searchlight_or_missile_launch"].as_f64().unwrap_or(0.); }
        if !f.under_fire {
            shifts += modifiers["firing_ship_not_under_fire"].as_f64().unwrap();
        }
        if f.local_control && f.standard != GunneryStandard::Manual {
            shifts += modifiers["local_control"].as_f64().unwrap();
        }
        if e.speed_kts > modifiers["shooter_speed_above_kts"].as_f64().unwrap() {
            shifts += modifiers["shooter_speed_modifier"].as_f64().unwrap();
        }
        if !f.radar_gfc {
            if let Some(row) = modifiers["visibility_max_percent"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| vis <= r[0].as_f64().unwrap())
            {
                shifts += row[1].as_f64().unwrap();
            }
        }
        let sea = env.sea_state;
        let size = dr::size(&e);
        let column = if matches!(size, "A" | "B") {
            0
        } else if matches!(size, "C" | "D") {
            1
        } else {
            2
        };
        let sea_mod = if sea < 3 {
            Some(0.)
        } else {
            tables()["sea_modifiers"][sea.to_string()][column].as_f64()
        };
        let Some(sea_mod) = sea_mod else {
            trace(
                w,
                j.actor,
                "8.3.no_fire",
                json!({"reason":"source sea-state NFP"}),
            );
            continue;
        };
        shifts += sea_mod;
        if t.speed_kts == 0. {
            shifts += modifiers["target_speed"]["stationary"].as_f64().unwrap();
        } else if t.speed_kts <= 10. {
            shifts += modifiers["target_speed"]["10_and_less_nonstationary"]
                .as_f64()
                .unwrap();
        } else if t.speed_kts >= 40. {
            shifts += modifiers["target_speed"]["40_and_above"].as_f64().unwrap();
        } else if t.speed_kts >= 30. {
            shifts += modifiers["target_speed"]["30_through39"].as_f64().unwrap();
        }
        let angle = crate::geom::angle_diff(t.course_deg, t.pos.bearing_to(e.pos)).abs();
        let narrow = tables()["aspect"]["narrow_max_deg"].as_f64().unwrap();
        let broad = tables()["aspect"]["broad_min_deg"].as_f64().unwrap();
        let aspect = if angle <= narrow || angle >= 180. - narrow {
            2
        } else if (broad..=180. - broad).contains(&angle) {
            0
        } else {
            1
        };
        let target_size = dr::size(&t);
        let aspect_key = match target_size {
            "A" => "A",
            "B" => "B",
            "C" | "D" => "CD",
            _ => "EG",
        };
        shifts += modifiers["target_aspect"][aspect_key][aspect]
            .as_f64()
            .unwrap();
        let mm = tables()["guns"][&j.weapon]["caliber_mm"].as_f64().unwrap();
        if b >= 2 {
            shifts -= groups[&(j.target, shell_group(mm))].len().saturating_sub(1) as f64;
        }
        let point_blank = vis
            >= modifiers["point_blank"]["visibility_min_percent"]
                .as_f64()
                .unwrap()
            && range
                <= ((tables()["guns"][&j.weapon]["bands"][0]["max_range_kyd"]
                    .as_f64()
                    .unwrap()
                    * 1000.
                    / 2.)
                    .floor()
                    / 2000.);
        if point_blank {
            shifts += modifiers["point_blank"]["modifier"].as_f64().unwrap();
        }
        let probability = (tables()["base_probability"][standard_key(f.standard)][b]
            .as_f64()
            .unwrap()
            + shifts * tables()["modifier_per_band"][b].as_f64().unwrap())
        .clamp(0., tables()["probability_maximum"].as_f64().unwrap());
        let identity = format!("8.3:{}:{}:{}:{}", j.tick, j.actor.0, j.weapon, j.target.0);
        let mut rng = Rng::new(w.seed).derive(Rng::hash_tag(j.tick, j.actor.0, &identity));
        let die = 1 + (rng.next_f64() * 100.) as u8;
        let hit = dr::threshold_met(probability * 100.,f64::from(die));
        if j.starshell {
            let mut miss_rng=Rng::new(w.seed).derive(Rng::hash_tag(j.tick,j.actor.0,&format!("{identity}:starshell_miss")));
            let miss=1+(miss_rng.next_f64()*100.) as u8;
            let effect=crate::rules::detection2026::starshell_effect(mm,range*2.,hit,miss);
            trace(w,j.actor,"5.8.8.starshell_resolution",json!({"target":j.target,"d100":die,"probability":probability,"hit":hit,"miss_d100":if hit {None}else{Some(miss)},"result":effect.as_ref().ok(),"pending":effect.as_ref().err()}));
            if let Ok(effect)=effect {
                let bearing=e.pos.bearing_to(t.pos);
                let forward=crate::Vec2::from_bearing(bearing);
                let lateral=crate::Vec2::from_bearing(bearing+90.);
                let point=t.pos.add(forward.scaled(effect.offset_kyd.y/2.)).add(lateral.scaled(effect.offset_kyd.x/2.));
                let works=effect.works;
                let physical_index=starshell_physical(w,j.actor,&j.weapon,&f).unwrap();
                w.rules.aaw.as_mut().unwrap().surface.gunnery.illumination.insert(slot(j.actor,&j.weapon),Illumination{actor:j.actor,target:j.target,weapon:j.weapon.clone(),physical_index,turn:tac(w.tick),effect,light_position:Some(point)});
                if works && w.day_night==crate::types::DayNight::Night && w.rules.detection.as_ref().and_then(|s|s.config.environment.as_ref()).is_some_and(|env|env.precipitation==0) {
                    let observers:Vec<_>=w.entities.values().filter(|o|o.alive && o.kind==PlatformKind::Ship && o.pos.range_to(point)<=30.).map(|o|(o.id,o.side)).collect();
                    for (observer,side) in observers {w.emit(EventKind::RulesEvent{domain:"detection2026".into(),rule:"5.8.8.light_seen".into(),side:Some(side),unit:Some(observer),contact:None,detail:json!({"kind":"starshell_light","at":w.tick}).to_string()},Visibility::Side(side));}
                }
            }
            continue;
        }
        let row = &tables()["guns"][&j.weapon]["bands"][b];
        let mult = tables()["damage_multiplier_by_max_barrels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r[0].as_u64().unwrap() >= u64::from(f.barrels))
            .unwrap()[b + 1]
            .as_f64()
            .unwrap();
        let damage = row["damage"].as_f64().unwrap()
            * mult
            * if point_blank {
                modifiers["point_blank"]["damage_factor"].as_f64().unwrap()
            } else {
                1.
            }
            * if f.reaction_only { 0.5 } else { 1. };
        trace(
            w,
            j.actor,
            "8.3.source_resolution",
            json!({"identity":identity,"target":j.target,"d100":die,"probability":probability,"shifts":shifts,"range_band":b,"hit":hit,"damage_before_armor":damage,"point_blank":point_blank}),
        );
        if hit {
            let p = dr::platform_of(&t).unwrap();
            let belt = row["belt_pen_cm"].as_f64().unwrap();
            let deck = row["deck_pen_cm"].as_f64();
            if b >= 2 && deck.is_some_and(|d| (d > p.deck_cm) != (belt > p.belt_cm)) {
                trace(
                    w,
                    j.actor,
                    "8.3.damage_pending",
                    json!({"identity":identity,"reason":"mixed belt/deck armor-face outcome unqualified; original hit die retained"}),
                );
                continue;
            }
            let mut def = w.db.weapons[&j.weapon].clone();
            def.damage = damage;
            def.name = tables()["guns"][&j.weapon]["name"].as_str().unwrap().into();
            def.rule_data = Some(dr::WeaponDamage {
                ship_dp: Some(damage),
                penetration_cm: Some(if b >= 2 { deck.unwrap_or(belt) } else { belt }),
                profile: if b < 2 { "skim" } else { "level" }.into(),
                diameter_mm: mm,
                source: "English2026 8.3 and exact declared Annex C shell/range band".into(),
                ..Default::default()
            });
            let proof = GunImpact {
                actor: j.actor,
                tactical_turn: tac(j.tick),
                range_band: b,
                caliber_mm: mm,
            };
            w.rules
                .aaw
                .as_mut()
                .unwrap()
                .surface
                .gunnery
                .impacts
                .insert(format!("{}:{identity}", j.target.0), proof);
            dr::queue(
                w,
                j.target,
                &def,
                false,
                Some(angle),
                Some(e.side),
                Some(identity),
            );
        }
    }
}

//! v4 exact source ASCM chain. Implemented only after two real Fire reds.
//! Stable exact ID us_rgm84d_ic is v4-only; generic IDs do not inherit it.
use crate::{db::WeaponDef, world::ProjectileData};
use crate::{ContactId, Database, EntityId, Side, Vec2, World};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const EXACT_IC_ID: &str = "us_rgm84d_ic";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Plot {
        contact: ContactId,
        weapon: String,
        course_deg: f64,
        speed_kts: f64,
        enable_run_nm: f64,
        waypoints: Vec<Vec2>,
        popup: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plot {
    pub actor: EntityId,
    pub side: Side,
    pub plotted_at: i64,
    pub contact: ContactId,
    pub weapon: String,
    pub course_deg: f64,
    pub speed_kts: f64,
    pub enable_run_nm: f64,
    pub waypoints: Vec<Vec2>,
    pub popup: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Configuration {
    pub source: String,
    pub launchers: Vec<LauncherBinding>,
    pub author_fcs: Vec<AuthorFcs>,
    pub author_awareness: Vec<AuthorAwareness>,
    pub author_countermeasure_availability: Vec<AuthorAvailability>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LauncherBinding {
    pub actor_name: String,
    pub weapon: String,
    pub exact_variant: String,
    pub max_rounds_per_turn: u8,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorFcs {
    pub actor_name: String,
    pub contact: ContactId,
    pub weapon: String,
    pub quality: super::super::firecontrol2026::Quality,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorAwareness {
    pub defender_name: String,
    pub shooter_name: String,
    pub weapon: String,
    pub from: i64,
    pub until: Option<i64>,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorAvailability {
    pub defender_name: String,
    pub jamming_available: bool,
    pub decoy_available: bool,
    pub source: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub config: Configuration,
    pub plots: BTreeMap<String, Plot>,
    pub salvos: BTreeMap<String, Salvo>,
    pub flights: BTreeMap<EntityId, Flight>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Salvo {
    pub identity: String,
    pub actor: EntityId,
    pub weapon: String,
    pub source_turn: i64,
    pub members: Vec<EntityId>,
    pub quality: super::super::firecontrol2026::Quality,
    pub placement: Option<super::super::firecontrol2026::Placement>,
    pub pending: Option<Pending>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Cruise,
    AwaitPlacementDetection,
    Search,
    LockedApproach,
    MissContinue,
    Pending,
    ImpactQueued,
    Intercepted,
    Runout,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateRef {
    Ship(EntityId),
    EwObject(EntityId),
    Unbound(EntityId),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub candidate: CandidateRef,
    pub actual_position: Vec2,
    pub range_nm: f64,
    pub bearing_error_deg: f64,
    pub source_profile: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LockTrial {
    pub identity: String,
    pub at: i64,
    pub candidate: CandidateRef,
    pub source_signature: usize,
    pub source_seeker_generation: u8,
    pub source_ecm_generation: Option<u8>,
    pub countermeasure_mode: String,
    pub awareness_evidence: Value,
    pub probability: f64,
    pub d100: u8,
    pub locked: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pending {
    pub at: i64,
    pub phase: String,
    pub code: String,
    pub reason: String,
    pub source_refs: Vec<String>,
    pub candidates: Vec<Candidate>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Flight {
    pub projectile: EntityId,
    pub actor: EntityId,
    pub side: Side,
    pub weapon: String,
    pub selected_contact: ContactId,
    pub intended_target: EntityId,
    pub launched_at: i64,
    pub source_turn: i64,
    pub origin: Vec2,
    pub own_reported_position: Vec2,
    pub initial_distance_nm: f64,
    pub plotted: Plot,
    pub fcs_source_snapshot: Value,
    pub source_weapon_snapshot: Value,
    pub salvo: String,
    pub phase: Phase,
    pub actual_run_nm: f64,
    pub remaining_range_nm: f64,
    pub actual_course_deg: f64,
    pub half_crossed_at: Option<i64>,
    pub placement: Option<super::super::firecontrol2026::Placement>,
    pub deviation_applied: bool,
    pub enabled_at: Option<i64>,
    pub locked_target: Option<CandidateRef>,
    pub lock_trials: Vec<LockTrial>,
    pub attacked: Vec<CandidateRef>,
    pub last_attack_position: Option<Vec2>,
    pub last_attack_course_deg: Option<f64>,
    pub reattacks: u16,
    pub damage_queued: bool,
    pub pending: Option<Pending>,
}

/// No Deserialize or public constructor. Player JSON cannot forge approval.
pub struct ReleasePermit {
    plot: Plot,
    target_from_own_contact: EntityId,
    own_aim: Vec2,
    rounds: u8,
    fcs_snapshot: Value,
    source_weapon_snapshot: Value,
}
impl ReleasePermit {
    pub fn actor(&self) -> EntityId {
        self.plot.actor
    }
    pub fn side(&self) -> Side {
        self.plot.side
    }
    pub fn contact(&self) -> ContactId {
        self.plot.contact
    }
    pub fn reported_target(&self) -> EntityId {
        self.target_from_own_contact
    }
    pub fn weapon(&self) -> &str {
        &self.plot.weapon
    }
    pub fn rounds(&self) -> u8 {
        self.rounds
    }
    pub fn own_aim(&self) -> Vec2 {
        self.own_aim
    }
    pub fn course_deg(&self) -> f64 {
        self.plot.course_deg
    }
    pub fn speed_kts(&self) -> f64 {
        self.plot.speed_kts
    }
    pub fn source_weapon(&self) -> &Value {
        &self.source_weapon_snapshot
    }
}

pub enum Dispatch {
    NotThisSourceDomain,
    Handled,
}

fn active(w: &World) -> bool {
    w.rules.profile == super::super::damage_runtime::RuleProfile::Rts2026Full
}
pub(crate) fn source_domain(w: &World, def: &WeaponDef) -> bool {
    active(w)
        && def.kind == crate::db::WeaponKind::Ascm
        && def.guidance == crate::db::Guidance::SeaSkimmer
        && super::super::damage_runtime::prh_generation(def) == 0
        && super::super::ground2026::source_missile(w, &def.id).is_err()
}
/// Register the exact stable variant only when the v4 loader calls this.
/// Generic us_harpoon/us_agm84 and all historical tables remain untouched.
pub fn configure_database(db: &mut Database) {
    let row = &super::tables()["weapons"][EXACT_IC_ID];
    let def: WeaponDef = serde_json::from_value(serde_json::json!({
        "id":EXACT_IC_ID,"name":"RGM-84D Harpoon IC",
        "kind":"Ascm","targets":{"surface":true},"range_nm":row["range_nm"],
        "min_range_nm":row["min_range_nm"],"speed_kts":row["speed_kts"],
        "pk":0.0,"salvo":1,"reload_s":0.0,"damage":40.0,
        "guidance":"SeaSkimmer","rule_data":row["damage"]
    }))
    .expect("registered exact source IC row");
    db.weapons.insert(EXACT_IC_ID.into(), def);
}

fn author(source: &str) -> bool {
    source == "scenario_assumption" || source.starts_with("scenario_assumption:")
}
fn plot_key(actor: EntityId, weapon: &str) -> String {
    format!("{}:{weapon}", actor.0)
}
fn salvo_key(actor: EntityId, weapon: &str, turn: i64) -> String {
    format!("6.3.8.ascm:{}:{weapon}:{turn}", actor.0)
}
fn state(w: &World) -> Result<&State, String> {
    Ok(&w
        .rules
        .aaw
        .as_ref()
        .ok_or("8.4 source runtime missing")?
        .surface
        .antiship)
}
fn take_state(w: &mut World) -> State {
    std::mem::take(&mut w.rules.aaw.as_mut().unwrap().surface.antiship)
}
fn put_state(w: &mut World, s: State) {
    w.rules.aaw.as_mut().unwrap().surface.antiship = s;
}
fn trace(w: &mut World, id: EntityId, rule: &str, value: Value) {
    w.emit(
        crate::EventKind::RulesEvent {
            domain: "antiship2026".into(),
            rule: rule.into(),
            side: None,
            unit: Some(id),
            contact: None,
            detail: value.to_string(),
        },
        crate::Visibility::Referee,
    );
}
fn weapon_row(weapon: &str) -> Result<&'static Value, String> {
    super::tables()["weapons"]
        .get(weapon)
        .filter(|v| v["source"].as_str().is_some_and(|s| !s.is_empty()))
        .ok_or_else(|| {
            "8.4 exact source missile variant missing; generic IDs do not inherit IC".into()
        })
}
fn number(v: &Value, key: &str) -> Result<f64, String> {
    v[key]
        .as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("8.4 required source {key} missing"))
}
pub fn initialize(w: &mut World, config: Option<Configuration>) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let config = config.unwrap_or_default();
    for l in &config.launchers {
        if !author(&l.source)
            || l.max_rounds_per_turn == 0
            || l.weapon != EXACT_IC_ID
            || l.exact_variant != weapon_row(&l.weapon)?["variant"].as_str().unwrap()
            || w.entities
                .values()
                .filter(|e| e.name == l.actor_name)
                .count()
                != 1
        {
            return Err("8.4 initial exact launcher requires unique actor and explicit complete scenario_assumption fit/rate".into());
        }
    }
    if config.author_fcs.iter().any(|a| !author(&a.source))
        || config.author_awareness.iter().any(|a| !author(&a.source))
        || config
            .author_countermeasure_availability
            .iter()
            .any(|a| !author(&a.source))
    {
        return Err(
            "8.4 author knowledge/FCS/availability require explicit scenario_assumption provenance"
                .into(),
        );
    }
    w.rules
        .aaw
        .as_mut()
        .ok_or("8.4 source runtime missing")?
        .surface
        .antiship
        .config = config;
    Ok(())
}
/// Check only the actor's own platform, orders and selected side contact.
/// Callers can reject invalid orders before transferring manual control.
pub fn validate_plot(w: &World, actor: EntityId, action: &Action) -> Result<(), String> {
    if !active(w) {
        return Err("8.4 source Plot requires v4".into());
    }
    let e = w
        .entity(actor)
        .filter(|e| e.alive)
        .ok_or("8.4 own actor unavailable")?;
    let Action::Plot {
        contact,
        weapon,
        course_deg,
        speed_kts,
        enable_run_nm,
        waypoints,
        popup,
    } = action;
    let row = weapon_row(weapon)?;
    let c = w
        .contact(e.side, *contact)
        .filter(|c| !c.stale && !c.bearing_only && c.category == crate::types::PlatformKind::Ship)
        .ok_or("6.3.7 selected own current located ship report required")?;
    if super::super::detection2026::known_category(w, e.side, c.about)
        != Some(crate::types::PlatformKind::Ship)
    {
        return Err("6.3.7 selected ship classification is not known to this side".into());
    }
    let aim = c.estimate_at(w.tick);
    if !e.weapons.iter().any(|m| m.weapon == *weapon)
        || !aim.x.is_finite()
        || !aim.y.is_finite()
        || !course_deg.is_finite()
        || !speed_kts.is_finite()
        || !enable_run_nm.is_finite()
        || *speed_kts != number(row, "speed_kts")?
        || *enable_run_nm < 0.
        || *enable_run_nm > number(row, "range_nm")?
        || waypoints
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return Err(
            "6.3.7 own fitted exact weapon and finite source flight orders required".into(),
        );
    }
    if *popup || !waypoints.is_empty() {
        return Err("7.4.8 popup/waypoint execution outside current no-popup source subset; no legacy fallback".into());
    }
    let range = e.pos.range_to(aim);
    if range < number(row, "min_range_nm")?
        || range > number(row, "range_nm")?
        || *enable_run_nm < range / 2.
    {
        return Err(
            "6.3.7/6.3.8 own reported range or early enable outside current source subset".into(),
        );
    }
    state(w)?;
    Ok(())
}
pub fn apply(w: &mut World, actor: EntityId, action: &Action) -> Result<(), String> {
    validate_plot(w, actor, action)?;
    let e = w.entity(actor).ok_or("8.4 own actor unavailable")?;
    let Action::Plot {
        contact,
        weapon,
        course_deg,
        speed_kts,
        enable_run_nm,
        waypoints,
        popup,
    } = action;
    let plot = Plot {
        actor,
        side: e.side,
        plotted_at: w.tick,
        contact: *contact,
        weapon: weapon.clone(),
        course_deg: crate::geom::normalize_deg(*course_deg),
        speed_kts: *speed_kts,
        enable_run_nm: *enable_run_nm,
        waypoints: waypoints.clone(),
        popup: *popup,
    };
    w.rules
        .aaw
        .as_mut()
        .ok_or("8.4 source runtime missing")?
        .surface
        .antiship
        .plots
        .insert(plot_key(actor, weapon), plot);
    Ok(())
}
fn prepare(
    w: &World,
    actor: EntityId,
    contact: ContactId,
    weapon: &str,
    rounds: u8,
) -> Result<ReleasePermit, String> {
    let e = w
        .entity(actor)
        .filter(|e| e.alive && super::super::damage_runtime::weapons_operational(e))
        .ok_or("8.4 own launcher unavailable")?;
    if e.kind != crate::types::PlatformKind::Ship {
        return Err(
            "8.4 source RGM84D surface launch only; alternate aircraft/sub variant unbound".into(),
        );
    }
    let row = weapon_row(weapon)?;
    let m = e
        .weapons
        .iter()
        .find(|m| m.weapon == weapon && m.mounts > 0 && m.cooldown_s <= 0.)
        .ok_or("8.4 own physical mount not ready")?;
    if rounds == 0 || u16::from(rounds) > m.ammo {
        return Err("8.4 requested physical rounds unavailable".into());
    }
    let c = w
        .contact(e.side, contact)
        .filter(|c| !c.stale && !c.bearing_only && c.category == crate::types::PlatformKind::Ship)
        .ok_or("8.4 selected own current located ship report required")?;
    if super::super::detection2026::known_category(w, e.side, c.about)
        != Some(crate::types::PlatformKind::Ship)
    {
        return Err("8.4 selected ship classification is not known to this side".into());
    }
    let aim = c.estimate_at(w.tick);
    if !aim.x.is_finite() || !aim.y.is_finite() {
        return Err("8.4 selected own location is not finite".into());
    }
    let s = state(w)?;
    let bindings: Vec<_> = s
        .config
        .launchers
        .iter()
        .filter(|l| l.actor_name == e.name && l.weapon == weapon)
        .collect();
    if bindings.len() != 1 {
        return Err("8.4 explicit own exact variant launcher fit/rate required".into());
    }
    let plot = s
        .plots
        .get(&plot_key(actor, weapon))
        .filter(|p| p.actor == actor && p.contact == contact && p.weapon == weapon)
        .cloned()
        .ok_or("6.3.7 selected own contact must match the actual Plot")?;
    let range = e.pos.range_to(aim);
    if range < number(row, "min_range_nm")? || range > number(row, "range_nm")? {
        return Err("8.4 own reported range outside exact source envelope".into());
    }
    if plot.popup || !plot.waypoints.is_empty() || plot.enable_run_nm < range / 2. {
        return Err(
            "6.3.8/7.4 early enable, popup or waypoint combination awaits source execution".into(),
        );
    }
    let declared: Vec<_> = s
        .config
        .author_fcs
        .iter()
        .filter(|a| a.actor_name == e.name && a.weapon == weapon && a.contact == contact)
        .collect();
    let fcs_snapshot = if declared.len() == 1 && author(&declared[0].source) {
        serde_json::json!({"quality":declared[0].quality,"source":declared[0].source,"actor":actor,"contact":contact,"own_reported_position":aim})
    } else if declared.is_empty() {
        let q = super::super::firecontrol2026::attack_solution(
            w,
            actor,
            c.about,
            row["seeker_generation"].as_u64().unwrap() as usize,
            plot.speed_kts,
        )?;
        serde_json::json!({"quality":q.quality,"solution":q,"source":"6.3 native/qualified received/composed source solution","actor":actor,"contact":contact,"own_reported_position":aim})
    } else {
        return Err("6.3 ambiguous author FCS declaration".into());
    };
    let quality =
        super::super::firecontrol2026::Quality::parse(fcs_snapshot["quality"].as_str().unwrap())?;
    if quality == super::super::firecontrol2026::Quality::NA {
        return Err("6.3 no attack solution".into());
    }
    let key = salvo_key(actor, weapon, w.tick.div_euclid(180));
    if let Some(group) = s.salvos.get(&key) {
        if group.quality != quality || group.pending.is_some() {
            return Err(
                "6.3.8 mixed-quality/source-pending salvo cannot borrow an earlier placement"
                    .into(),
            );
        }
        if let Some(prior) = group.members.first().and_then(|id| s.flights.get(id)) {
            if prior.plotted.course_deg != plot.course_deg
                || prior.plotted.enable_run_nm != plot.enable_run_nm
            {
                return Err("6.3.8 mixed plotted routes in one source salvo are unresolved".into());
            }
        }
        if group.members.len() + rounds as usize > bindings[0].max_rounds_per_turn as usize {
            return Err("7.1 declared own launcher turn budget exhausted".into());
        }
    } else if rounds > bindings[0].max_rounds_per_turn {
        return Err("7.1 declared own launcher turn budget exceeded".into());
    }
    Ok(ReleasePermit {
        plot,
        target_from_own_contact: c.about,
        own_aim: aim,
        rounds,
        fcs_snapshot,
        source_weapon_snapshot: row.clone(),
    })
}
pub fn fire_at_contact(
    w: &mut World,
    actor: EntityId,
    contact: ContactId,
    weapon: &str,
    rounds: u8,
) -> Option<Result<(), String>> {
    if !active(w)
        || weapon != EXACT_IC_ID
            && !w
                .db
                .weapons
                .get(weapon)
                .is_some_and(|d| source_domain(w, d))
    {
        return None;
    }
    Some(
        prepare(w, actor, contact, weapon, rounds)
            .and_then(|permit| crate::weapons::launch_source_antiship(w, &permit).map(|_| ())),
    )
}
pub fn register_launch(w: &mut World, projectile: EntityId, permit: &ReleasePermit) {
    let origin = w.entity(permit.actor()).unwrap().pos;
    let initial_run = w.entity(projectile).unwrap().pos.range_to(origin);
    let turn = w.tick.div_euclid(180);
    let key = salvo_key(permit.actor(), permit.weapon(), turn);
    let quality = super::super::firecontrol2026::Quality::parse(
        permit.fcs_snapshot["quality"].as_str().unwrap(),
    )
    .unwrap();
    let f = Flight {
        projectile,
        actor: permit.actor(),
        side: permit.side(),
        weapon: permit.weapon().into(),
        selected_contact: permit.contact(),
        intended_target: permit.reported_target(),
        launched_at: w.tick,
        source_turn: turn,
        origin,
        own_reported_position: permit.own_aim(),
        initial_distance_nm: origin.range_to(permit.own_aim()),
        plotted: permit.plot.clone(),
        fcs_source_snapshot: permit.fcs_snapshot.clone(),
        source_weapon_snapshot: permit.source_weapon_snapshot.clone(),
        salvo: key.clone(),
        phase: Phase::Cruise,
        actual_run_nm: initial_run,
        remaining_range_nm: permit.source_weapon_snapshot["range_nm"].as_f64().unwrap()
            - initial_run,
        actual_course_deg: permit.course_deg(),
        half_crossed_at: None,
        placement: None,
        deviation_applied: false,
        enabled_at: None,
        locked_target: None,
        lock_trials: vec![],
        attacked: vec![],
        last_attack_position: None,
        last_attack_course_deg: None,
        reattacks: 0,
        damage_queued: false,
        pending: None,
    };
    let s = &mut w.rules.aaw.as_mut().unwrap().surface.antiship;
    s.salvos
        .entry(key.clone())
        .or_insert(Salvo {
            identity: key,
            actor: permit.actor(),
            weapon: permit.weapon().into(),
            source_turn: turn,
            members: vec![],
            quality,
            placement: None,
            pending: None,
        })
        .members
        .push(projectile);
    s.flights.insert(projectile, f);
}
fn pending(f: &mut Flight, w: &World, code: &str, reason: &str, candidates: Vec<Candidate>) {
    if f.pending.is_none() {
        f.pending = Some(Pending {
            at: w.tick,
            phase: format!("{:?}", f.phase),
            code: code.into(),
            reason: reason.into(),
            source_refs: vec!["6.3.8/7.4/8.4.1".into()],
            candidates,
        });
    }
    f.phase = Phase::Pending;
}
fn retain_pending(w: &mut World, f: &Flight) {
    if w.rules.pending_adjudications.contains_key(&f.projectile) {
        return;
    }
    trace(
        w,
        f.projectile,
        "8.4.source_pending",
        serde_json::json!({"flight":f}),
    );
    if let Some(body) = w.entity(f.projectile).cloned() {
        let evidence=w.events.iter().filter(|e|matches!(&e.kind,crate::EventKind::RulesEvent{domain,unit,..}if domain=="antiship2026"&&*unit==Some(f.projectile))).cloned().collect();
        w.rules.pending_adjudications.insert(
            f.projectile,
            super::super::damage_runtime::PendingAdjudication {
                tick: w.tick,
                rule: f.pending.as_ref().unwrap().reason.clone(),
                target: f.intended_target,
                projectile: body,
                evidence,
            },
        );
    }
}
pub fn after_detection(w: &mut World) {
    if !active(w) {
        return;
    }
    let mut s = take_state(w);
    let mut fcs = w.rules.aaw.as_ref().unwrap().firecontrol.clone();
    for f in s
        .flights
        .values_mut()
        .filter(|f| f.half_crossed_at.is_some() && f.placement.is_none() && f.pending.is_none())
    {
        let group = s.salvos.get_mut(&f.salvo).unwrap();
        let p = if let Some(p) = &group.placement {
            Ok(p.clone())
        } else {
            super::super::firecontrol2026::placement(w, &mut fcs, &group.identity, group.quality, 0)
        };
        match p {
            Ok(p) => {
                group.placement = Some(p.clone());
                f.placement = Some(p.clone());
                f.deviation_applied = true;
                f.actual_course_deg = crate::geom::normalize_deg(
                    f.plotted.course_deg + p.course_deviation_deg as f64,
                );
                if let Some(body) = w.entity_mut(f.projectile) {
                    body.course_deg = f.actual_course_deg;
                    body.ordered_course_deg = f.actual_course_deg;
                }
                f.phase = Phase::Search;
                trace(
                    w,
                    f.projectile,
                    "6.3.8.source_placement",
                    serde_json::json!({"salvo":group.identity,"placement":p,"course_deg":f.actual_course_deg,"first_detection_after_half":w.tick}),
                );
            }
            Err(e) => pending(f, w, "placement_source", &e, vec![]),
        }
    }
    w.rules.aaw.as_mut().unwrap().firecontrol = fcs;
    let held: Vec<_> = s
        .flights
        .values()
        .filter(|f| f.pending.is_some())
        .cloned()
        .collect();
    put_state(w, s);
    for f in held {
        retain_pending(w, &f);
    }
}
pub fn table_probability(
    signature: &str,
    ecm: Option<u8>,
    seeker: u8,
    mode: &str,
) -> Result<f64, String> {
    if !(1..=4).contains(&seeker) || ecm.is_some_and(|g| !(1..=4).contains(&g)) {
        return Err("8.4.1 unsupported source generation".into());
    }
    let column = match mode {
        "base" => 2,
        "jamming" => 3,
        "decoy" => 4,
        "jamming_decoy" => 5,
        _ => return Err("8.4.1 unknown countermeasure mode".into()),
    };
    if column != 2 && ecm.is_none() {
        return Err("8.4.1 source ECM generation absent".into());
    }
    let rows = super::tables()["antiship"]
        .get(signature)
        .and_then(Value::as_array)
        .ok_or("8.4.1 source signature absent")?;
    rows.iter()
        .find(|r| {
            r[1].as_u64() == Some(seeker as u64)
                && (column == 2 || r[0].as_u64() == ecm.map(u64::from))
        })
        .and_then(|r| r[column].as_f64())
        .ok_or_else(|| "8.4.1 exact printed cell absent".into())
}
fn acquisition(f: &Flight, signature: usize) -> Result<f64, String> {
    let generation = f.source_weapon_snapshot["seeker_generation"]
        .as_u64()
        .ok_or("7.4 source seeker generation missing")? as usize;
    if !(1..=4).contains(&generation) || signature > 4 {
        return Err("7.4 source acquisition inputs unsupported".into());
    }
    let column = if signature <= 1 { 0 } else { signature - 1 };
    super::tables()["acquisition_nm"]["TARH"][generation - 1][column]
        .as_f64()
        .ok_or_else(|| "7.4 acquisition cell missing".into())
}
fn clear_environment(w: &World) -> Result<(), String> {
    let env = w
        .context
        .radar_environment
        .as_ref()
        .filter(|e| e.valid())
        .ok_or("7.4.4 explicit source clutter environment required")?;
    if env.sea_state != 0
        || env.precipitation != "None"
        || w.context.terrain.is_some()
        || w.context
            .radar_land_clutter
            .as_ref()
            .is_none_or(|r| !r.is_empty())
    {
        return Err("7.4.4 non-clear source seeker clutter combination awaits execution; no full-range default".into());
    }
    Ok(())
}
fn candidates(w: &World, f: &Flight) -> Result<Vec<Candidate>, String> {
    clear_environment(w)?;
    let body = w.entity(f.projectile).ok_or("7.4 missile body absent")?;
    let mut found = vec![];
    let max = acquisition(f, 0)?;
    let half = super::tables()["seeker_half_angle_deg"].as_f64().unwrap();
    for t in w
        .entities
        .values()
        .filter(|e| e.alive && e.kind == crate::types::PlatformKind::Ship)
    {
        if f.attacked.contains(&CandidateRef::Ship(t.id))
            || f.placement.as_ref().is_some_and(|p| !p.placed) && t.id == f.intended_target
        {
            continue;
        }
        let range = body.pos.range_to(t.pos);
        let error = crate::geom::angle_diff(body.pos.bearing_to(t.pos), f.actual_course_deg).abs();
        if range > max || error > half {
            continue;
        }
        let row = super::tables()["platforms"].get(&t.class);
        let signature = row
            .and_then(|p| p["signature_index"].as_u64())
            .map(|n| n as usize);
        if signature.is_some_and(|sig| acquisition(f, sig).is_ok_and(|r| range > r)) {
            continue;
        }
        found.push(Candidate {
            candidate: if row.is_some() {
                CandidateRef::Ship(t.id)
            } else {
                CandidateRef::Unbound(t.id)
            },
            actual_position: t.pos,
            range_nm: range,
            bearing_error_deg: error,
            source_profile: row.cloned(),
        });
    }
    if let Some(ew) = w.rules.ew.as_ref() {
        for object in ew.objects.values().filter(|o| {
            o.effective(w.tick)
                && matches!(
                    o.kind,
                    super::super::ew2026::ObjectKind::LongRangeChaff
                        | super::super::ew2026::ObjectKind::FloatingAsm
                )
        }) {
            let range = body.pos.range_to(object.pos);
            let error =
                crate::geom::angle_diff(body.pos.bearing_to(object.pos), f.actual_course_deg).abs();
            if range <= max && error <= half {
                found.push(Candidate {
                    candidate: CandidateRef::EwObject(object.id),
                    actual_position: object.pos,
                    range_nm: range,
                    bearing_error_deg: error,
                    source_profile: Some(serde_json::json!({"object":object})),
                });
            }
        }
    }
    Ok(found)
}
fn lock(w: &mut World, s: &State, f: &mut Flight, candidate: &Candidate) -> Result<(), String> {
    let CandidateRef::Ship(target) = candidate.candidate else {
        return Err("7.4 alternate/unbound/EW candidate execution pending".into());
    };
    if target != f.intended_target {
        return Err(
            "7.3/7.4 other-ship acquisition pending; not silently nearest-target selection".into(),
        );
    }
    let t = w.entity(target).ok_or("8.4 physical target absent")?;
    let row = candidate
        .source_profile
        .as_ref()
        .ok_or("8.4 target source profile missing")?;
    if t.systems.radar < 1. || t.hp < t.hp_max {
        return Err("8.4 damaged defender ECM operational mapping unverified".into());
    }
    let owner = w.entity(f.actor).map(|e| e.name.as_str()).unwrap_or("");
    let aware: Vec<_> = s
        .config
        .author_awareness
        .iter()
        .filter(|a| {
            a.defender_name == t.name
                && a.shooter_name == owner
                && a.weapon == f.weapon
                && w.tick >= a.from
                && a.until.is_none_or(|until| w.tick < until)
        })
        .collect();
    if aware.len() != 1 {
        return Err(
            "8.4.1 explicit source/author evidence defender knows under attack missing".into(),
        );
    }
    let availability: Vec<_> = s
        .config
        .author_countermeasure_availability
        .iter()
        .filter(|a| a.defender_name == t.name)
        .collect();
    if availability.len() > 1 {
        return Err("8.4 ambiguous defender availability".into());
    }
    let j = row["jamming"]
        .as_bool()
        .ok_or("8.4 source jamming capability missing")?
        && availability.first().is_none_or(|a| a.jamming_available);
    let d = row["decoy"]
        .as_bool()
        .ok_or("8.4 source decoy capability missing")?
        && availability.first().is_none_or(|a| a.decoy_available);
    let mode = match (j, d) {
        (false, false) => "base",
        (true, false) => "jamming",
        (false, true) => "decoy",
        (true, true) => "jamming_decoy",
    };
    let seeker = f.source_weapon_snapshot["seeker_generation"]
        .as_u64()
        .unwrap() as u8;
    let ecm = row["ecm_generation"].as_u64().map(|n| n as u8);
    let probability = table_probability(
        row["signature"].as_str().ok_or("8.4 signature absent")?,
        ecm,
        seeker,
        mode,
    )?;
    let identity = format!("8.4.1.ascm:{}:{}", f.projectile.0, f.lock_trials.len());
    let mut rng = w.rng.derive(crate::Rng::hash_tag(0, 0, &identity));
    let d100 = 1 + (rng.next_f64() * 100.) as u8;
    let locked = d100 as f64 <= probability * 100.;
    f.lock_trials.push(LockTrial{identity,at:w.tick,candidate:candidate.candidate,source_signature:row["signature_index"].as_u64().unwrap() as usize,source_seeker_generation:seeker,
        source_ecm_generation:ecm,countermeasure_mode:mode.into(),awareness_evidence:serde_json::json!({"source":aware[0].source,"from":aware[0].from,"until":aware[0].until}),probability,d100,locked});
    f.attacked.push(candidate.candidate);
    f.last_attack_position = Some(candidate.actual_position);
    f.last_attack_course_deg = Some(f.actual_course_deg);
    if locked {
        f.locked_target = Some(candidate.candidate);
        f.phase = Phase::LockedApproach;
        super::super::nuclear2026::seeker_activated(w,f.projectile,true);
    } else {
        f.phase = Phase::MissContinue;
        if super::super::nuclear2026::terminal(w,f.projectile,false){f.phase=Phase::Runout;retire(w,f.projectile);}
    }
    trace(
        w,
        f.projectile,
        "8.4.1.source_lock",
        serde_json::json!({"trial":f.lock_trials.last(),"physical_position":w.entity(f.projectile).map(|e|e.pos)}),
    );
    Ok(())
}
fn move_body(w: &mut World, f: &mut Flight, distance: f64, course: f64) {
    f.actual_course_deg = crate::geom::normalize_deg(course);
    f.actual_run_nm += distance;
    f.remaining_range_nm = (f.remaining_range_nm - distance).max(0.);
    if let Some(body) = w.entity_mut(f.projectile) {
        body.pos = body.pos.advanced(f.actual_course_deg, distance);
        body.course_deg = f.actual_course_deg;
        body.ordered_course_deg = f.actual_course_deg;
        body.ext
            .insert("flight.distance_nm".into(), f.actual_run_nm);
        if let Some(data) = body.projectile.as_mut() {
            data.live_s += 1.;
            data.enabled = f.enabled_at.is_some();
        }
    }
}
fn retire(w: &mut World, id: EntityId) {
    let now = w.tick;
    if let Some(e) = w.entity_mut(id) {
        e.alive = false;
        e.hp = 0.;
        e.last_emit_tick = now - 1000;
    }
}
fn advance_flight(w: &mut World, s: &State, f: &mut Flight, def: &WeaponDef) -> Result<(), String> {
    if matches!(
        f.phase,
        Phase::Pending | Phase::ImpactQueued | Phase::Intercepted | Phase::Runout
    ) {
        return Ok(());
    }
    if w.entity(f.projectile).is_none_or(|e| !e.alive) {
        f.phase = Phase::Intercepted;
        return Ok(());
    }
    let step = (f.plotted.speed_kts / 3600.).min(f.remaining_range_nm);
    if f.phase == Phase::LockedApproach {
        let Some(CandidateRef::Ship(target)) = f.locked_target else {
            return Err("8.4 locked physical reference missing".into());
        };
        let target_body = w
            .entity(target)
            .filter(|e| e.alive && e.kind == crate::types::PlatformKind::Ship);
        if target_body.is_none()&&super::super::nuclear2026::removed(w,f.projectile,super::super::nuclear2026::Removal::LostAcquiredLock){f.phase=Phase::Runout;retire(w,f.projectile);return Ok(());}
        let t=w.entity(target).filter(|e|e.alive&&e.kind==crate::types::PlatformKind::Ship).ok_or("7.4 locked target lost/changed domain; source continuation pending")?;
        let position = w.entity(f.projectile).unwrap().pos;
        let range = position.range_to(t.pos);
        let course = position.bearing_to(t.pos);
        move_body(w, f, step.min(range), course);
        if range <= step {
            crate::damage::apply_projectile_hit(w, f.projectile, target, def, false, f.side);
            f.damage_queued = true;
            f.phase = Phase::ImpactQueued;
            retire(w, f.projectile);
            trace(
                w,
                f.projectile,
                "8.4.1.actual_impact",
                serde_json::json!({"target":target,"flight":f}),
            );
        }
    } else {
        let course = f.actual_course_deg;
        move_body(w, f, step, course);
        if f.placement.is_none() && f.actual_run_nm >= f.initial_distance_nm / 2. {
            f.half_crossed_at.get_or_insert(w.tick);
            f.phase = Phase::AwaitPlacementDetection;
        }
        if f.placement.is_some() && f.actual_run_nm >= f.plotted.enable_run_nm {
            f.enabled_at.get_or_insert(w.tick);
            super::super::nuclear2026::seeker_activated(w,f.projectile,false);
            let found = candidates(w, f)?;
            if found.len() > 1 {
                pending(f,w,"multiple_candidates","7.4 random multi-target/decoy combination pending; complete candidates retained",found);
            } else if let Some(c) = found.first() {
                if let Err(e) = lock(w, s, f, c) {
                    pending(f, w, "candidate_source", &e, found);
                }
            } else if f.placement.as_ref().is_some_and(|p| p.placed)
                && !f.attacked.contains(&CandidateRef::Ship(f.intended_target))
                && w.entity(f.projectile)
                    .unwrap()
                    .pos
                    .range_to(f.own_reported_position)
                    <= acquisition(f, 0)?
            {
                pending(f,w,"placement_geometry","7.4 source placed target not physically acquired in actual cone; no truth steering or invented point",vec![]);
            }
        }
    }
    if f.remaining_range_nm <= 0. && !matches!(f.phase, Phase::Pending | Phase::ImpactQueued) {
        f.phase = Phase::Runout;
        retire(w, f.projectile);
    }
    Ok(())
}
pub fn advance(
    w: &mut World,
    projectile: EntityId,
    def: &WeaponDef,
    data: &ProjectileData,
) -> Dispatch {
    if !source_domain(w, def) {
        return Dispatch::NotThisSourceDomain;
    }
    let mut s = take_state(w);
    if let Some(mut f) = s.flights.remove(&projectile) {
        if let Err(e) = advance_flight(w, &s, &mut f, def) {
            pending(&mut f, w, "source_missing", &e, vec![]);
        }
        let held = f.pending.is_some();
        s.flights.insert(projectile, f.clone());
        put_state(w, s);
        if held {
            retain_pending(w, &f);
        }
    } else {
        put_state(w, s);
        if let Some(body) = w.entity(projectile).cloned() {
            w.rules.pending_adjudications.entry(projectile).or_insert(
                super::super::damage_runtime::PendingAdjudication {
                    tick: w.tick,
                    rule: "8.4 source ASCM lacks full registered flight; no legacy fallback".into(),
                    target: data.target.unwrap_or(projectile),
                    projectile: body,
                    evidence: vec![],
                },
            );
        }
    }
    Dispatch::Handled
}
pub fn on_intercept(w: &mut World, projectile: EntityId) {
    super::super::nuclear2026::removed(w,projectile,super::super::nuclear2026::Removal::CarrierDestroyed);
    if active(w) {
        if let Some(f) = w
            .rules
            .aaw
            .as_mut()
            .and_then(|s| s.surface.antiship.flights.get_mut(&projectile))
        {
            if !f.damage_queued {
                f.phase = Phase::Intercepted;
            }
        }
    }
}
pub fn own_view(w: &World, actor: EntityId) -> Option<Value> {
    if !active(w) {
        return None;
    }
    let s = state(w).ok()?;
    let plots: Vec<_> = s.plots.values().filter(|p| p.actor == actor).collect();
    Some(serde_json::json!({"profile":"hv2026-rts-v4","plots":plots,"execution":"source_executor"}))
}
pub fn incoming_radar_signature(w: &World, weapon: &str) -> Option<usize> {
    (active(w) && weapon == EXACT_IC_ID).then_some(3)
}
pub fn incoming_aaw_profile(
    w: &World,
    projectile: EntityId,
) -> Result<Option<&'static Value>, String> {
    if active(w)
        && w.entity(projectile)
            .and_then(|e| e.projectile.as_ref())
            .is_some_and(|p| p.weapon == EXACT_IC_ID)
    {
        let flight = state(w)?
            .flights
            .get(&projectile)
            .filter(|f| {
                f.projectile == projectile
                    && f.weapon == EXACT_IC_ID
                    && f.plotted.weapon == EXACT_IC_ID
                    && !f.plotted.popup
            })
            .ok_or("8.1 exact IC incoming needs an actual registered no-popup source Flight; MR remains unresolved")?;
        let profile = weapon_row(&flight.weapon)?
            .get("no_popup_incoming_profile")
            .filter(|p| p.is_object())
            .ok_or("8.1 no-popup IC AA profile unavailable; strict source adjudication pending")?;
        Ok(Some(profile))
    } else {
        Ok(None)
    }
}
pub fn incoming_detection_profile(
    w: &World,
    weapon: &str,
) -> Result<Option<super::super::detection2026::TargetProfile>, String> {
    if active(w) && weapon == EXACT_IC_ID {
        Err("chapter5 exact IC physical size unverified; radar VSmall is not physical size".into())
    } else {
        Ok(None)
    }
}

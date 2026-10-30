//! Source 10.2.7: one qualified opportunity after failed reacquisition.
//! A post-Movement snapshot is an explicit RTS epoch: the Search tick has
//! already consumed its complete old-setting movement and endurance. Capture
//! never adds movement/attack to that tick; fastest-setting pursuit starts
//! on the next tick. No allegiance filter or distance/ID target priority is
//! inferred. Unknown/multiple/vertical decisions remain referee pending.
use super::*;
use crate::Side;

const REVISION: &str = "2026-10-25-unique-source-opportunity-first-attack-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Opportunity {
    pub object: EntityId,
    pub kind: OpportunityKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u8>,
    pub pass: u32,
    /// Historical opportunity states cannot acquire invented provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<OpportunityCapture>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpportunityKind {
    Entity,
    MobileDecoy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpportunityCapture {
    pub revision: String,
    pub owner: Side,
    pub release_target: EntityId,
    pub captured_at: i64,
    pub position: Vec2,
    pub depth_m: f64,
    pub search_speed_index: usize,
    pub attack_speed_index: usize,
    pub original_placement: Placement,
    pub candidate_placement: Placement,
    pub inputs: CaptureInputs,
    pub pursuit_started_at: Option<i64>,
    pub last_advanced_at: Option<i64>,
    pub threat: Option<Threat>,
    pub terminal: Option<OpportunityAttack>,
    pub pending: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureInputs {
    pub source: String,
    pub profile: Target,
    /// The immutable fitted identity is separate from damage at capture.
    /// A later real hit may change effective noise/coating without rerolling
    /// this already-paid placement or replacing the captured object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_profile: Option<Target>,
    pub combat_kind: PlatformKind,
    pub target_position: Vec2,
    pub target_depth_m: f64,
    pub active: bool,
    pub cross_layer: bool,
    pub acquisition_nm: f64,
    pub range_nm: f64,
    pub off_axis_deg: f64,
    pub placement_modifier: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpportunityAttack {
    pub tick: i64,
    pub target: EntityId,
    pub d100: u8,
    pub probability: f64,
    pub hit: bool,
    pub evading: bool,
    pub acm_generation: Option<usize>,
}

struct Candidate {
    id: EntityId,
    kind: OpportunityKind,
    generation: Option<u8>,
    inputs: CaptureInputs,
}

fn finite_position(p: Vec2) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

fn emit(w: &mut World, id: EntityId, owner: Side, rule: &str, detail: Value) {
    w.emit(
        crate::EventKind::RulesEvent {
            domain: "torpedo2026".into(),
            rule: rule.into(),
            side: Some(owner),
            unit: Some(id),
            contact: None,
            detail: detail.to_string(),
        },
        crate::Visibility::Referee,
    );
}

fn profile_valid(profile: &Target) -> bool {
    !profile.source.trim().is_empty() && profile.size <= 6 && profile.noise <= 4
}

fn same_profile(a: &Target, b: &Target) -> bool {
    a.source == b.source
        && a.size == b.size
        && a.noise == b.noise
        && a.anechoic == b.anechoic
        && a.foil_borne == b.foil_borne
}

fn source_environment(state: &State) -> Result<&Environment, String> {
    state
        .config
        .environment
        .as_ref()
        .filter(|env| {
            !env.source.trim().is_empty()
                && env
                    .layer_m
                    .is_none_or(|depth| depth.is_finite() && depth > 0.)
        })
        .ok_or_else(|| "10.2.7 opportunity source environment missing/invalid".into())
}

fn modifier(f: &Flight, env: &Environment, profile: &Target) -> i32 {
    let coating = if profile.anechoic && f.weapon.guidance != Guidance::Passive {
        -2
    } else {
        0
    };
    coating
        + if env.reverberant && !f.weapon.shallow_water_capable {
            if f.weapon.generation <= 3 {
                -2
            } else {
                -1
            }
        } else {
            0
        }
}

/// Test each candidate's own physical profile. Distinct depths are never
/// projected onto the torpedo's plane or teleported on capture.
fn inputs(
    body: &crate::world::Entity,
    f: &Flight,
    env: &Environment,
    source: String,
    profile: Target,
    kind: PlatformKind,
    pos: Vec2,
    depth: f64,
) -> Result<Option<CaptureInputs>, String> {
    if !finite_position(pos) || !depth.is_finite() || depth < 0. {
        return Err("candidate physical position/depth missing".into());
    }
    if !profile_valid(&profile) {
        return Err("candidate size/noise/coating source missing or invalid".into());
    }
    if profile.foil_borne || depth > f.weapon.max_depth_m {
        return Ok(None);
    }
    let active = f.weapon.guidance != Guidance::Passive && f.run_nm >= f.order.enable_run_nm;
    let cross = env
        .layer_m
        .is_some_and(|layer| (body.depth_m > layer) != (depth > layer));
    let acquisition = acquisition_nm(
        f.weapon.generation,
        active,
        profile.noise,
        cross,
        env.reverberant,
        f.weapon.shallow_water_capable,
    )?;
    let range = body.pos.range_to(pos);
    let off_axis = angle_diff(body.course_deg, body.pos.bearing_to(pos)).abs();
    if range > acquisition || off_axis > 60. {
        return Ok(None);
    }
    if depth != body.depth_m {
        return Err(
            "candidate within source range/cone is outside the coplanar pursuit subset".into(),
        );
    }
    Ok(Some(CaptureInputs {
        source,
        source_profile: None,
        placement_modifier: modifier(f, env, &profile),
        profile,
        combat_kind: kind,
        target_position: pos,
        target_depth_m: depth,
        active,
        cross_layer: cross,
        acquisition_nm: acquisition,
        range_nm: range,
        off_axis_deg: off_axis,
    }))
}

fn mobile_profile(obj: &crate::rules::ew2026::Object) -> Result<Target, String> {
    if obj.source.trim().is_empty() || !obj.generation.is_some_and(|g| (1..=4).contains(&g)) {
        return Err("mobile simulator source/ACM generation missing".into());
    }
    let imitated = obj
        .imitated_profile
        .as_ref()
        .filter(|profile| !profile.source.trim().is_empty())
        .ok_or("mobile simulator imitated acoustic profile missing")?;
    Ok(Target {
        source: format!(
            "{}; imitated acoustic noise: {}",
            obj.source, imitated.source
        ),
        size: imitated.size,
        noise: imitated
            .noise
            .ok_or("mobile simulator imitated noise missing")?,
        // A simulated echo of a coated submarine is not an anechoic coating
        // on the simulator. Do not inherit the original target's -2 modifier.
        anechoic: false,
        foil_borne: false,
    })
}

/// Called after the complete Search movement/endurance tick and runout test.
/// A return of true records a capture only; the caller must end this tick's
/// generic attack processing. Multiple/unknown candidates draw no dice.
pub(super) fn scan(
    w: &mut World,
    state: &mut State,
    id: EntityId,
    f: &mut Flight,
    release_target: EntityId,
) -> Result<bool, String> {
    if f.opportunity.is_some()
        || f.pass == 0
        || !matches!(f.phase, Phase::Search { .. })
        || !f.placement.as_ref().is_some_and(|p| !p.placed)
    {
        return Ok(false);
    }
    let body = w
        .entity(id)
        .filter(|e| e.alive)
        .cloned()
        .ok_or("10.2.7 torpedo body missing")?;
    if body.kind != PlatformKind::Torpedo
        || !finite_position(body.pos)
        || !body.depth_m.is_finite()
        || body.depth_m < 0.
        || body.depth_m > f.weapon.max_depth_m
        || !f.fuel.is_finite()
        || !(0. ..1.).contains(&f.fuel)
    {
        return Err("10.2.7 opportunity body/depth/endurance invalid".into());
    }
    let env = source_environment(state)?.clone();
    let active = f.weapon.guidance != Guidance::Passive && f.run_nm >= f.order.enable_run_nm;
    let outer = acquisition_nm(f.weapon.generation, active, 0, false, false, true)?;
    let possible = |pos: Vec2| {
        !finite_position(pos)
            || (body.pos.range_to(pos) <= outer
                && angle_diff(body.course_deg, body.pos.bearing_to(pos)).abs() <= 60.)
    };
    let mut qualified = Vec::new();
    let mut unknown = Vec::new();
    // Physical acoustic capture has no side-based IFF. A friendly/launcher
    // body in this region needs the same source checks as any other target.
    for target in w
        .entities
        .values()
        .filter(|e| e.alive && e.id != id && e.id != release_target)
    {
        let kind = target.combat_kind();
        if !((kind == PlatformKind::Sub && f.weapon.submarine)
            || (kind == PlatformKind::Ship && f.weapon.surface))
            || !possible(target.pos)
        {
            continue;
        }
        match state
            .config
            .targets
            .get(&target.class)
            .cloned()
            .ok_or_else(|| {
                "candidate explicit torpedo-domain size/noise/coating/foil source missing"
                    .to_string()
            })
            .and_then(|profile| {
                let effective = effective_target_spec(w, target.id, profile.clone())?;
                let mut captured = inputs(
                    &body,
                    f,
                    &env,
                    profile.source.clone(),
                    effective,
                    kind,
                    target.pos,
                    target.depth_m,
                )?;
                if let Some(inputs) = &mut captured {
                    inputs.source_profile = Some(profile);
                }
                Ok(captured)
            }) {
            Ok(Some(inputs)) => qualified.push(Candidate {
                id: target.id,
                kind: OpportunityKind::Entity,
                generation: None,
                inputs,
            }),
            Ok(None) => {}
            Err(reason) => {
                unknown.push(json!({"candidate":target.id,"kind":"entity","reason":reason}))
            }
        }
    }
    if let Some(ew) = &w.rules.ew {
        for obj in ew.objects.values().filter(|obj| {
            obj.kind == crate::rules::ew2026::ObjectKind::MobileDecoy
                && obj.effective(w.tick)
                && f.weapon.submarine
                && possible(obj.pos)
        }) {
            match mobile_profile(obj).and_then(|profile| {
                inputs(
                    &body,
                    f,
                    &env,
                    obj.source.clone(),
                    profile,
                    PlatformKind::Sub,
                    obj.pos,
                    obj.depth_m,
                )
            }) {
                Ok(Some(inputs)) => qualified.push(Candidate {
                    id: obj.id,
                    kind: OpportunityKind::MobileDecoy,
                    generation: obj.generation,
                    inputs,
                }),
                Ok(None) => {}
                Err(reason) => {
                    unknown.push(json!({"candidate":obj.id,"kind":"mobile_decoy","reason":reason}))
                }
            }
        }
    }
    if !unknown.is_empty() || qualified.len() > 1 {
        emit(
            w,
            id,
            body.side,
            "10.2.7.opportunity_pending",
            json!({
            "torpedo":id,"pass":f.pass,"unknown":unknown,
            "qualified":qualified.iter().map(|c|json!({"candidate":c.id,"kind":c.kind,"inputs":c.inputs})).collect::<Vec<_>>(),
            "policy":"no target priority, IFF or vertical inference"}),
        );
        return Err("10.2.7 multiple/unknown opportunity candidates need a source adjudication; no nearest/ID priority inferred".into());
    }
    let Some(candidate) = qualified.pop() else {
        return Ok(false);
    };
    if f.wire {
        return Err("10.2.7 opportunity capture with live manual wire steering requires a bound override policy".into());
    }
    let identity = format!("10.placement:{}:{}:op:{}", id.0, f.pass, candidate.id.0);
    let already = state.placements.placements.contains_key(&identity);
    let placement = super::super::firecontrol2026::placement(
        w,
        &mut state.placements,
        &identity,
        f.quality,
        candidate.inputs.placement_modifier,
    )?;
    if !already {
        emit(
            w,
            id,
            body.side,
            "10.2.4.opportunity_placement",
            json!({
            "torpedo":id,"pass":f.pass,"candidate":candidate.id,"kind":candidate.kind,
            "range_nm":candidate.inputs.range_nm,"inputs":candidate.inputs,
            "placement":placement,"identity":identity}),
        );
    }
    if !placement.placed {
        return Ok(false);
    }
    let fastest = f
        .weapon
        .settings
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.speed_kts.is_finite() && s.speed_kts > 0. && s.range_nm.is_finite() && s.range_nm > 0.
        })
        .max_by(|(_, a), (_, b)| a.speed_kts.total_cmp(&b.speed_kts))
        .map(|(i, _)| i)
        .ok_or("7.5.7 opportunity fastest source setting missing")?;
    let capture = OpportunityCapture {
        revision: REVISION.into(),
        owner: body.side,
        release_target,
        captured_at: w.tick,
        position: body.pos,
        depth_m: body.depth_m,
        search_speed_index: f.speed_index,
        attack_speed_index: fastest,
        original_placement: f
            .placement
            .clone()
            .ok_or("10.2.7 original failed Placement missing")?,
        candidate_placement: placement,
        inputs: candidate.inputs,
        pursuit_started_at: None,
        last_advanced_at: None,
        threat: None,
        terminal: None,
        pending: None,
    };
    emit(
        w,
        id,
        body.side,
        "10.2.7.opportunity_acquired",
        json!({
        "torpedo":id,"pass":f.pass,"candidate":candidate.id,"kind":candidate.kind,
        "capture":capture}),
    );
    f.opportunity = Some(Opportunity {
        object: candidate.id,
        kind: candidate.kind,
        generation: candidate.generation,
        pass: f.pass,
        capture: Some(capture),
    });
    f.phase = Phase::Locked;
    Ok(true)
}

/// Capture the real defender's pose before its next native command/reaction.
/// An original-target threat record is never borrowed for this defender.
pub(super) fn begin(w: &mut World) {
    if !active(w) {
        return;
    }
    let snapshots: Vec<_> = w.rules.torpedoes.as_ref().map_or_else(Vec::new, |state| {
        state
            .flights
            .iter()
            .filter_map(|(id, f)| {
                let op = f
                    .opportunity
                    .as_ref()
                    .filter(|op| op.kind == OpportunityKind::Entity)?;
                let capture = op
                    .capture
                    .as_ref()
                    .filter(|a| a.threat.is_none() && a.terminal.is_none())?;
                let _ = capture;
                let target = w.entity(op.object).filter(|e| e.alive)?;
                if crate::rules::detection2026::known_category(w, target.side, *id)
                    != Some(PlatformKind::Torpedo)
                    || !w.sides[target.side.index()]
                        .contacts
                        .values()
                        .any(|c| c.about == *id && !c.stale)
                {
                    return None;
                }
                Some((
                    *id,
                    Threat {
                        course: target.course_deg,
                        speed: target.speed_kts,
                        depth_band: depth_band(target.depth_m).ok()?,
                    },
                ))
            })
            .collect()
    });
    if let Some(state) = w.rules.torpedoes.as_mut() {
        for (id, threat) in snapshots {
            if let Some(capture) = state
                .flights
                .get_mut(&id)
                .and_then(|f| f.opportunity.as_mut())
                .and_then(|op| op.capture.as_mut())
            {
                capture.threat.get_or_insert(threat);
            }
        }
    }
}

pub(super) fn advance(
    w: &mut World,
    state: &mut State,
    id: EntityId,
    def: &WeaponDef,
    f: &mut Flight,
) -> Result<(), String> {
    let mut op = f
        .opportunity
        .take()
        .ok_or("10.2.7 opportunity record missing")?;
    let result = advance_inner(w, state, id, def, f, &mut op);
    if let Err(reason) = &result {
        if let Some(capture) = op.capture.as_mut() {
            capture.pending = Some(reason.clone());
        }
    }
    f.opportunity = Some(op);
    result
}

fn advance_inner(
    w: &mut World,
    state: &mut State,
    id: EntityId,
    def: &WeaponDef,
    f: &mut Flight,
    op: &mut Opportunity,
) -> Result<(), String> {
    let capture = op.capture.as_mut().ok_or(
        "10.2.7 historical opportunity lacks source capture provenance; no silent migration",
    )?;
    if capture.revision != REVISION || capture.release_target != f.target || op.pass != f.pass {
        return Err("10.2.7 saved opportunity revision/release identity mismatch".into());
    }
    if capture.terminal.is_some() {
        return Err("10.2.7 opportunity first-attack result retained; subsequent target/research decision pending without another die".into());
    }
    if w.tick <= capture.captured_at || capture.last_advanced_at.is_some_and(|tick| tick >= w.tick)
    {
        return Ok(());
    }
    if f.wire {
        return Err(
            "10.2.7 live-wire opportunity pursuit requires a source override policy".into(),
        );
    }
    let body = w
        .entity(id)
        .filter(|e| e.alive)
        .cloned()
        .ok_or("10.2.7 captured torpedo body missing")?;
    if body.side != capture.owner || body.depth_m != capture.depth_m || !finite_position(body.pos) {
        return Err(
            "10.2.7 saved owner/depth/body invalid; no launcher-side fallback or vertical teleport"
                .into(),
        );
    }
    let (target_pos, target_depth, entity) = match op.kind {
        OpportunityKind::Entity => {
            let target = w
                .entity(op.object)
                .filter(|e| e.alive)
                .cloned()
                .ok_or("10.2.7 opportunity entity lost; reassignment pending")?;
            let source = state
                .config
                .targets
                .get(&target.class)
                .filter(|p| !p.source.trim().is_empty())
                .ok_or("10.2.7 captured entity explicit torpedo profile source missing")?;
            let profile = effective_target_spec(w, target.id, source.clone())?;
            if !same_profile(
                source,
                capture
                    .inputs
                    .source_profile
                    .as_ref()
                    .unwrap_or(&capture.inputs.profile),
            ) || profile.foil_borne
                || target.combat_kind() != capture.inputs.combat_kind
            {
                return Err(
                    "10.2.7 captured entity source/domain changed; complete decision pending"
                        .into(),
                );
            }
            if !state.depth_motion.get(&target.id).is_some_and(|d| {
                d.tick == w.tick && d.from == Some(capture.depth_m) && d.to == capture.depth_m
            }) {
                return Err("10.2.7 opportunity vertical pursuit beyond complete coplanar depth history is pending".into());
            }
            if capture.threat.is_none()
                && crate::rules::detection2026::known_category(w, target.side, id)
                    == Some(PlatformKind::Torpedo)
                && w.sides[target.side.index()]
                    .contacts
                    .values()
                    .any(|c| c.about == id && !c.stale)
            {
                return Err("10.2.5 opportunity defender missing precommand baseline; late reaction snapshot forbidden".into());
            }
            (target.pos, target.depth_m, Some(target))
        }
        OpportunityKind::MobileDecoy => {
            let obj = w
                .rules
                .ew
                .as_ref()
                .and_then(|s| s.objects.get(&op.object))
                .filter(|o| {
                    o.kind == crate::rules::ew2026::ObjectKind::MobileDecoy && o.effective(w.tick)
                })
                .ok_or("10.2.7 captured mobile simulator expired/lost; reassignment pending")?;
            let profile = mobile_profile(obj)?;
            if obj.generation != op.generation
                || obj.source != capture.inputs.source
                || !same_profile(&profile, &capture.inputs.profile)
            {
                return Err(
                    "10.2.7 captured mobile simulator source/ACM generation changed".into(),
                );
            }
            (obj.pos, obj.depth_m, None)
        }
    };
    if !finite_position(target_pos)
        || target_depth != capture.depth_m
        || target_depth > f.weapon.max_depth_m
    {
        return Err("10.2.7 opportunity target left qualified plane/depth; vertical steering/research pending".into());
    }
    let setting = f
        .weapon
        .settings
        .get(capture.attack_speed_index)
        .filter(|s| {
            s.speed_kts.is_finite() && s.speed_kts > 0. && s.range_nm.is_finite() && s.range_nm > 0.
        })
        .ok_or("7.5.7 opportunity fastest setting invalid")?;
    if !f.fuel.is_finite() || !(0. ..=1.).contains(&f.fuel) {
        return Err("10.2.7 saved opportunity endurance invalid".into());
    }
    let speed = setting.speed_kts / 3600.;
    let remaining = (1. - f.fuel) * setting.range_nm;
    let affordable = speed.min(remaining);
    let distance = body.pos.range_to(target_pos);
    let reached = distance <= affordable;
    let travel = affordable.min(distance);
    let course = body.pos.bearing_to(target_pos);
    let pos = body.pos.advanced(course, travel);
    f.speed_index = capture.attack_speed_index;
    f.run_nm += travel;
    f.fuel = (f.fuel + travel / setting.range_nm).min(1.);
    capture.last_advanced_at = Some(w.tick);
    capture.pursuit_started_at.get_or_insert(w.tick);
    if let Some(e) = w.entity_mut(id) {
        e.pos = pos;
        e.course_deg = course;
        e.speed_kts = setting.speed_kts;
        // Keep actual coplanar depth. An order's old depth is not a target.
        if let Some(projectile) = e.projectile.as_mut() {
            projectile.live_s += travel / speed;
            projectile.enabled = capture.inputs.active;
        }
    }
    if !reached {
        if remaining <= speed {
            f.fuel = 1.;
            emit(
                w,
                id,
                capture.owner,
                "10.2.source_runout",
                json!({"torpedo":id,"spent_fraction":f.fuel,"opportunity":op.object}),
            );
            retire(w, id);
        }
        return Ok(());
    }
    let (evading, acm) = if let Some(target) = &entity {
        (
            capture
                .threat
                .as_ref()
                .is_some_and(|threat| evasion(threat, target, false)),
            crate::rules::ew2026::active_acms(w, target.id)
                .iter()
                .max_by_key(|a| a.generation)
                .map(|a| a.generation as usize),
        )
    } else {
        (
            false,
            Some(
                op.generation
                    .filter(|g| (1..=4).contains(g))
                    .ok_or("10.2.7 simulator ACM generation missing")? as usize,
            ),
        )
    };
    let probability = homing_probability(f.weapon.generation, acm, evading)?;
    let d100 = rng_die(w, id, &format!("10.2.5.attack:{}", f.pass), 100);
    let hit = source_percent_hit(probability, d100);
    f.last_probability = Some(probability);
    f.last_d100 = Some(d100);
    capture.terminal = Some(OpportunityAttack {
        tick: w.tick,
        target: op.object,
        d100,
        probability,
        hit,
        evading,
        acm_generation: acm,
    });
    emit(
        w,
        id,
        capture.owner,
        "10.2.5.source_terminal",
        json!({
        "torpedo":id,"pass":f.pass,"target":op.object,"opportunity":op.object,
        "opportunity_kind":op.kind,"d100":d100,"probability":probability,
        "evading":evading,"acm_generation":acm,"hit":hit,"capture":capture}),
    );
    if !hit {
        return Err("10.2.7 opportunity first attack missed; original D100 retained, choice of next target/reattack after opportunity requires source adjudication".into());
    }
    match op.kind {
        OpportunityKind::Entity => {
            crate::damage::apply_projectile_hit(w, id, op.object, def, true, capture.owner)
        }
        OpportunityKind::MobileDecoy => {
            if let Some(obj) = w
                .rules
                .ew
                .as_mut()
                .and_then(|s| s.objects.get_mut(&op.object))
            {
                obj.status = crate::rules::ew2026::Status::Destroyed;
            }
            emit(
                w,
                id,
                capture.owner,
                "10.2.7.opportunity_destroyed",
                json!({"torpedo":id,"pass":f.pass,"decoy":op.object}),
            );
        }
    }
    retire(w, id);
    Ok(())
}

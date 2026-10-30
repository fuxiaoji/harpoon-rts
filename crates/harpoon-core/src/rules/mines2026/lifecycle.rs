//! Source-bounded complete-row cohorts and already-classified individual hunts.
//! No search/classification probability or partial-field density is invented here.
use super::*;
const YD: f64 = 2025.3718285;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayFit {
    pub source: String,
    pub kind: String,
    pub variants: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tube_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MineStock {
    pub source: String,
    pub mines: BTreeMap<String, u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayPlan {
    pub source: String,
    pub unit: String,
    pub field: Field,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arming_s: Option<i64>,
    #[serde(default)]
    pub activation_delay_s: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayJob {
    pub plan_id: String,
    pub plan: LayPlan,
    pub fit: LayFit,
    pub points: Vec<Vec2>,
    pub visited: BTreeSet<usize>,
    pub worked_s: i64,
    pub last_tick: i64,
    pub previous: Vec2,
    pub combat_seen: bool,
    pub phase: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Body {
    pub id: String,
    pub field: String,
    pub mine: String,
    pub owner: Side,
    pub position: Vec2,
    pub case_band: u8,
    pub released_at: i64,
    pub armed_at: i64,
    #[serde(default)]
    pub activation_at: i64,
    #[serde(default)]
    pub neutralized: bool,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClassifiedMine {
    pub id: String,
    pub side: Side,
    pub mine_id: String,
    pub position: Vec2,
    pub case_band: u8,
    pub classified: bool,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HunterFit {
    pub source: String,
    pub method: String,
    pub range_yd: f64,
    pub max_depth_band: u8,
    pub max_work_speed_kts: f64,
    pub charges: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HuntJob {
    pub actor: EntityId,
    pub contact: String,
    pub d6: u8,
    pub required_s: i64,
    pub worked_s: i64,
    #[serde(default)]
    pub attempted_s: i64,
    pub last_tick: i64,
    pub phase: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartialPath {
    pub field: String,
    pub target: EntityId,
    pub tick: i64,
    pub from: Vec2,
    pub to: Vec2,
    pub danger_distance_yd: f64,
}
fn finite(p: Vec2) -> bool {
    p.x.is_finite() && p.y.is_finite()
}
fn points(f: &Field) -> Result<Vec<Vec2>, String> {
    if !f.area.valid() || f.spacing_yd == 0 || f.rows == 0 {
        return Err("13.4 invalid source row geometry".into());
    }
    let along = f.area.start.range_to(f.area.end) * YD / f.spacing_yd as f64;
    if (along - along.round()).abs() > 1e-6 || along > 100000. {
        return Err("13.4 fractional or oversized explicit mine manifest".into());
    }
    let d = f.area.end.sub(f.area.start);
    let len = d.norm();
    let mut result = vec![];
    for row in 0..f.rows {
        let offset = (row as f64 - (f.rows - 1) as f64 / 2.) * f.row_spacing_yd as f64 / YD;
        for i in 0..=along.round() as usize {
            let t = i as f64 / along;
            result.push(Vec2::new(
                f.area.start.x + d.x * t - d.y / len * offset,
                f.area.start.y + d.y * t + d.x / len * offset,
            ));
        }
    }
    Ok(result)
}
pub(super) fn initialize(w: &World, c: &Configuration, s: &mut State) -> Result<(), String> {
    let bound_names: BTreeSet<_> = c
        .carried_mines
        .keys()
        .chain(c.hunter_fits.keys())
        .chain(c.laying_fits.keys())
        .cloned()
        .chain(c.lay_plans.values().map(|p| p.unit.clone()))
        .collect();
    for name in bound_names {
        if w.entities
            .values()
            .filter(|e| !e.kind.is_projectile() && e.name == name)
            .count()
            != 1
        {
            return Err(
                "13.3 authored mine stock/fit/plan must bind exactly one physical unit name".into(),
            );
        }
    }
    // Trusted identities cannot collide with a hidden initial field or plan
    // and later turn into a player admission/availability probe.
    let mut field_ids: BTreeSet<_> = s.fields.keys().cloned().collect();
    for p in c.lay_plans.values() {
        if !field_ids.insert(p.field.id.clone()) {
            return Err(
                "13.4 source lay plan/field identities must be unique at initialization".into(),
            );
        }
    }
    for e in w.entities.values() {
        if let Some(stock) = c.carried_mines.get(&e.name) {
            if stock.source.trim().is_empty() {
                return Err("13.3 mine stock requires source friendly-forces quantity".into());
            }
            for id in stock.mines.keys() {
                mine(id)?;
            }
            s.stocks.insert(e.id, stock.mines.clone());
        }
        if let Some(fit) = c.hunter_fits.get(&e.name) {
            s.charges.insert(e.id, fit.charges);
        }
    }
    for b in &c.mine_bodies {
        if b.id.is_empty()
            || b.source.trim().is_empty()
            || !finite(b.position)
            || b.case_band > 12
            || b.armed_at < b.released_at
            || b.activation_at < 0
            || s.bodies.insert(b.id.clone(), b.clone()).is_some()
        {
            return Err("13.8 invalid/duplicate explicit mine body".into());
        }
        mine(&b.mine)?;
    }
    for (fid, ids) in &c.field_manifests {
        let f = s
            .fields
            .get(fid)
            .ok_or("13.8 manifest requires complete source Field")?;
        let expected = points(f)?;
        let mut used = BTreeSet::new();
        if ids.len() != expected.len() {
            return Err("13.4 full manifest must conserve exact source row mine count".into());
        }
        for id in ids {
            let b = s.bodies.get(id).ok_or("13.8 manifest mine body missing")?;
            let index = expected
                .iter()
                .position(|p| p.range_to(b.position) < 1e-8)
                .ok_or("13.4 body is not at declared source row position")?;
            if b.field != *fid
                || b.mine != f.mine
                || b.case_band != f.case_band
                || b.owner != f.owner
                || b.armed_at != f.armed_at
                || b.activation_at != f.activation_at
                || !used.insert(index)
            {
                return Err("13.4 manifest type/depth/position mismatch".into());
            }
        }
        s.manifests.insert(fid.clone(), ids.clone());
    }
    for b in s.bodies.values() {
        if !s
            .manifests
            .get(&b.field)
            .is_some_and(|ids| ids.contains(&b.id))
        {
            return Err("13.8 individual body requires complete linked Field manifest; standalone attack model unknown".into());
        }
    }
    let mut contact_ids = BTreeSet::new();
    let mut classified_sides: BTreeMap<String, Side> = BTreeMap::new();
    for report in &c.known_mine_contacts {
        if report.source.trim().is_empty()
            || report.id.is_empty()
            || !finite(report.position)
            || report.case_band > 12
            || !s.bodies.contains_key(&report.mine_id)
            || !contact_ids.insert(format!("{:?}:{}", report.side, report.id))
        {
            return Err("13.8 invalid authored classified mine contact".into());
        }
        if report.classified
            && classified_sides
                .insert(report.mine_id.clone(), report.side)
                .is_some_and(|side| side != report.side)
        {
            return Err("13.8 contested cross-side classified mine neutralization requires a separate private work/claim procedure; unsupported in this scenario subset".into());
        }
        s.classified.push(report.clone());
    }
    Ok(())
}
fn fit_and_plan(
    w: &World,
    actor: EntityId,
    plan_id: &str,
) -> Result<(LayFit, LayPlan, Vec<Vec2>), String> {
    let e = w
        .entity(actor)
        .filter(|e| e.alive)
        .ok_or("minelayer unavailable")?;
    let c = w
        .context
        .mines_2026
        .as_ref()
        .ok_or("13.3 source minelaying configuration missing")?;
    let fit = c
        .laying_fits
        .get(&e.name)
        .filter(|f| !f.source.trim().is_empty())
        .ok_or("13.7 source minelayer fit missing")?
        .clone();
    let plan = c
        .lay_plans
        .get(plan_id)
        .filter(|p| p.unit == e.name && !p.source.trim().is_empty())
        .ok_or("13.7 own source lay plan unavailable")?
        .clone();
    let f = &plan.field;
    let spec = mine(&f.mine)?;
    if !fit.variants.contains(&f.mine) || spec["weight_kg"].as_u64().is_none() {
        return Err("13.3 exact source mine variant and carried equipment are mandatory".into());
    }
    let method = if fit.kind == "sub" { "sub" } else { "ship" };
    if !spec["lay_methods"]
        .as_array()
        .is_some_and(|a| a.iter().any(|x| x == method))
    {
        return Err("13.7 AnnexG variant does not permit this laying method".into());
    }
    if f.rows != 1
        || !f.safe_channels.is_empty()
        || f.ship_counter != 0
        || f.crossings != 0
        || f.source.trim().is_empty()
        || f.owner != e.side
        || f.row_spacing_yd < 200
        || (f.area.width_yd - 2. * f.row_spacing_yd as f64).abs() > 1e-7
        || spec["kind"] != "bottom"
        || f.water_band != f.case_band
        || spec["max_case_band"]
            .as_u64()
            .is_none_or(|max| f.case_band as u64 > max)
        || f.armed_at != 0
        || f.activation_at != 0
    {
        return Err("13.4/13.7 first laying package requires one complete bottom-mine row, surveyed depth, no unknown counter/partial-field procedure".into());
    }
    if plan.activation_delay_s < 0
        || plan.activation_delay_s > n("lifecycle", "activation_max_s") as i64
    {
        return Err("13.7 activation delay must be explicit and no more than two weeks".into());
    }
    arming_seconds(f.case_band, plan.arming_s)?;
    probability(true, 0, f.spacing_yd)?;
    let pts = points(f)?;
    let quota = quota(&fit)?;
    if pts.len() > quota {
        return Err(
            "13.7 complete row exceeds one source batch quota; partial density unsupported".into(),
        );
    }
    let s = w.rules.mines.as_ref().ok_or("source mine state missing")?;
    let stock = s
        .stocks
        .get(&actor)
        .and_then(|s| s.get(&f.mine))
        .copied()
        .ok_or("13.3 own source mine quantity unknown")?;
    if stock < pts.len() as u32 {
        return Err("13.3 not enough carried source mines for complete row".into());
    }
    if fit.kind == "sub" {
        if e.kind != PlatformKind::Sub {
            return Err("13.7 submarine mine fit requires submarine actor".into());
        }
        if super::super::torpedo2026::mine_inventory(w, actor, &f.mine)? != stock {
            return Err(
                "13.3 mine total differs from same physical loaded/reserve/reload inventory".into(),
            );
        }
        let mut indexes = BTreeSet::new();
        for id in fit.tube_ids.iter().take(pts.len()) {
            let idx = super::super::torpedo2026::tube_ready(w, actor, id)?;
            if !indexes.insert(idx) {
                return Err("13.7 declared mine tube IDs alias the same physical tube".into());
            }
            if w.rules.torpedoes.as_ref().unwrap().tubes[&actor].tubes[idx]
                .loaded
                .as_deref()
                != Some(f.mine.as_str())
            {
                return Err("13.7 declared physical mine tube contains another weapon".into());
            }
        }
    } else if e.kind != PlatformKind::Ship {
        return Err(
            "13.7 surface source fit requires ship actor; aircraft package unresolved".into(),
        );
    }
    envelope(w, actor, &fit)?;
    Ok((fit, plan, pts))
}
fn quota(fit: &LayFit) -> Result<usize, String> {
    if fit.kind == "sub" {
        let distinct: BTreeSet<_> = fit.tube_ids.iter().collect();
        if distinct.len() != fit.tube_ids.len() || fit.tube_ids.is_empty() {
            return Err("13.7 explicit unique physical tube IDs required".into());
        }
        return Ok(fit.tube_ids.len());
    }
    tables()["lay"]["rates"][&fit.kind]
        .as_u64()
        .map(|n| n as usize)
        .ok_or("13.7 unknown source laying role; aircraft/mobile package unsupported".into())
}
fn envelope(w: &World, actor: EntityId, fit: &LayFit) -> Result<(), String> {
    let e = w
        .entity(actor)
        .filter(|e| e.alive)
        .ok_or("minelayer unavailable")?;
    let sea = w
        .context
        .radar_environment
        .as_ref()
        .filter(|e| e.valid())
        .ok_or("13.7 explicit source sea state required")?
        .sea_state;
    let submarine = fit.kind == "sub";
    let max = if submarine {
        n("lay", "sub_max_kts")
    } else {
        n("lay", "ship_max_kts")
    };
    let sea_max = if submarine && e.depth_m > 0. {
        n("lay", "sub_sea_max")
    } else {
        n("lay", "surface_sea_max")
    };
    if !e.speed_kts.is_finite() || e.speed_kts < 0. || e.speed_kts > max || sea as f64 > sea_max {
        return Err("13.7 source minelaying speed/sea-state envelope exceeded".into());
    }

    Ok(())
}
fn in_combat(w: &World, actor: EntityId) -> bool {
    w.rules
        .mines
        .as_ref()
        .and_then(|s| s.combat.get(&actor))
        .is_some_and(|t| w.tick - *t < n("lifecycle", "combat_window_s") as i64)
}
fn known(w: &World, actor: EntityId, contact: &str) -> Result<ClassifiedMine, String> {
    let e = w
        .entity(actor)
        .filter(|e| e.alive)
        .ok_or("hunter unavailable")?;
    w.rules
        .mines
        .as_ref()
        .and_then(|s| {
            s.classified.iter().find(|c| {
                c.side == e.side && c.id == contact && c.classified && !c.source.trim().is_empty()
            })
        })
        .cloned()
        .ok_or("13.8 own classified mine contact unavailable".into())
}
fn hunter(w: &World, actor: EntityId, c: &ClassifiedMine) -> Result<HunterFit, String> {
    let e = w
        .entity(actor)
        .filter(|e| e.alive)
        .ok_or("hunter unavailable")?;
    let fit = w
        .context
        .mines_2026
        .as_ref()
        .and_then(|c| c.hunter_fits.get(&e.name))
        .filter(|f| !f.source.trim().is_empty())
        .ok_or("13.8 source neutralizer fit missing")?;
    if e.kind != PlatformKind::Ship
        || !matches!(fit.method.as_str(), "rov" | "diver" | "sea_mammal")
        || !fit.range_yd.is_finite()
        || fit.range_yd <= 0.
        || !fit.max_work_speed_kts.is_finite()
        || fit.max_work_speed_kts < 0.
        || fit.max_depth_band > 12
        || c.case_band > fit.max_depth_band
        || e.pos.range_to(c.position) * YD > fit.range_yd
        || e.speed_kts > fit.max_work_speed_kts
        || !permissive(w, actor)
    {
        return Err("13.8 source neutralizer range/depth/work-speed/equipment/noncombat condition unavailable".into());
    }
    Ok(fit.clone())
}
pub(super) fn apply(w: &mut World, actor: EntityId, action: &Action) -> Result<(), String> {
    let e = w
        .entity(actor)
        .filter(|e| e.alive)
        .ok_or("mine warfare actor unavailable")?
        .clone();
    match action {
        Action::Lay { plan_id } => {
            let (fit, plan, pts) = fit_and_plan(w, actor, plan_id)?;
            let s = w.rules.mines.as_ref().unwrap();
            if s.lay_jobs.contains_key(&actor)
                || s.fields.contains_key(&plan.field.id)
                || s.manifests.contains_key(&plan.field.id)
                || s.gear.get(&actor).is_some_and(|g| !g.lost)
                || s.hunts
                    .values()
                    .any(|h| h.actor == actor && h.phase == "working")
            {
                return Err("13.7 task/field identity/equipment already occupied".into());
            }
            let visited = pts
                .iter()
                .enumerate()
                .filter_map(|(i, p)| (p.range_to(e.pos) < 1e-8).then_some(i))
                .collect();
            let combat_seen = in_combat(w, actor);
            if fit.kind != "sub" && combat_seen && pts.len() > quota(&fit)? / 2 {
                return Err("13.7 combat halves complete-row source quota".into());
            }
            w.rules.mines.as_mut().unwrap().lay_jobs.insert(
                actor,
                LayJob {
                    plan_id: plan_id.clone(),
                    plan,
                    fit,
                    points: pts,
                    visited,
                    worked_s: 0,
                    last_tick: w.tick,
                    previous: e.pos,
                    combat_seen,
                    phase: "working".into(),
                },
            );
        }
        Action::CancelLay => {
            w.rules
                .mines
                .as_mut()
                .unwrap()
                .lay_jobs
                .remove(&actor)
                .ok_or("no own active lay job")?;
        }
        Action::Hunt { contact_id } => {
            let c = known(w, actor, contact_id)?;
            let fit = hunter(w, actor, &c)?;
            let s = w.rules.mines.as_ref().unwrap();
            if s.lay_jobs.contains_key(&actor)
                || s.gear.get(&actor).is_some_and(|g| !g.lost)
                || s.hunts
                    .iter()
                    .any(|(id, h)| h.actor == actor && h.phase == "working" && *id != c.mine_id)
            {
                return Err("13.8 own equipment already working another task".into());
            }
            if let Some(old) = s.hunts.get(&c.mine_id) {
                if old.actor != actor || old.phase == "complete" {
                    return Err("13.8 own classified target already claimed or neutralized".into());
                }
                let j = w
                    .rules
                    .mines
                    .as_mut()
                    .unwrap()
                    .hunts
                    .get_mut(&c.mine_id)
                    .unwrap();
                j.phase = "working".into();
                j.last_tick = w.tick;
            } else {
                let remaining = s.charges.get(&actor).copied().unwrap_or(0);
                if remaining == 0 {
                    return Err("13.8 no source neutralization charge remaining".into());
                }
                // Keyed to original mine identity: cancel/retry cannot obtain a new die.
                let mut r = w.rng.derive(Rng::hash_tag(
                    0,
                    actor.0,
                    &format!("13.8.hunt:{}", c.mine_id),
                ));
                let die = (r.next_f64() * 6.).floor() as u8 + 1;
                let required_s = die as i64 * n("sweep", "hunt_die_seconds") as i64;
                let s = w.rules.mines.as_mut().unwrap();
                s.charges.insert(actor, remaining - 1);
                s.hunts.insert(
                    c.mine_id.clone(),
                    HuntJob {
                        actor,
                        contact: contact_id.clone(),
                        d6: die,
                        required_s,
                        worked_s: 0,
                        attempted_s: 0,
                        last_tick: w.tick,
                        phase: "working".into(),
                    },
                );
                referee(
                    w,
                    Some(actor),
                    "13.8.hunt_original_die",
                    json!({"contact":contact_id,"mine":c.mine_id,"d6":die,"required_s":required_s,"fit_source":fit.source}),
                );
            }
        }
        Action::CancelHunt => {
            let j = w
                .rules
                .mines
                .as_mut()
                .unwrap()
                .hunts
                .values_mut()
                .find(|h| h.actor == actor && h.phase == "working")
                .ok_or("no own working hunt")?;
            j.phase = "paused".into();
            j.last_tick = w.tick;
        }
        _ => unreachable!(),
    }
    referee(
        w,
        Some(actor),
        "13.lifecycle.order",
        json!({"action":action}),
    );
    Ok(())
}
fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let d = b.sub(a);
    let l = d.x * d.x + d.y * d.y;
    let t = if l <= 1e-20 {
        0.
    } else {
        ((p.x - a.x) * d.x + (p.y - a.y) * d.y) / l
    }
    .clamp(0., 1.);
    p.range_to(Vec2::new(a.x + t * d.x, a.y + t * d.y))
}
pub(super) fn step(w: &mut World) {
    let jobs: Vec<_> = w
        .rules
        .mines
        .as_ref()
        .unwrap()
        .lay_jobs
        .iter()
        .map(|(a, j)| (*a, j.clone()))
        .collect();
    for (actor, mut j) in jobs {
        let Some(e) = w.entity(actor).cloned() else {
            continue;
        };
        let dt = (w.tick - j.last_tick).max(0).min(1); // no off-screen fast-forward work
        let legal = envelope(w, actor, &j.fit).is_ok();
        if legal {
            for (i, p) in j.points.iter().enumerate() {
                if segment_distance(*p, j.previous, e.pos) < 1e-8 {
                    j.visited.insert(i);
                }
            }
            j.worked_s += dt;
            j.combat_seen |= in_combat(w, actor);
            j.phase = "working".into();
        } else {
            // A broken work window must be traversed again; old completed mines never disappear.
            j.worked_s = 0;
            j.visited.clear();
            j.combat_seen = false;
            j.phase = "paused_envelope".into();
        }
        j.previous = e.pos;
        j.last_tick = w.tick;
        let rate = quota(&j.fit).unwrap_or(0)
            / (if j.fit.kind != "sub" && j.combat_seen {
                2
            } else {
                1
            });
        let due = if j.fit.kind == "sub" {
            0
        } else {
            n("lay", "tactical_s") as i64
        };
        let ready = legal
            && j.worked_s >= due
            && j.visited.len() == j.points.len()
            && j.points.len() <= rate;
        if ready {
            match release(w, actor, &j) {
                Ok(()) => {
                    w.rules.mines.as_mut().unwrap().lay_jobs.remove(&actor);
                    continue;
                }
                Err(reason) => {
                    j.phase = "blocked_release".into();
                    diagnostic(w, actor, format!("lay:{}", j.plan_id), reason);
                }
            }
        } else if j.points.len() > rate {
            j.phase = "blocked_combat_quota".into();
        }
        w.rules.mines.as_mut().unwrap().lay_jobs.insert(actor, j);
    }
    let hunts: Vec<_> = w
        .rules
        .mines
        .as_ref()
        .unwrap()
        .hunts
        .iter()
        .map(|(id, j)| (id.clone(), j.clone()))
        .collect();
    for (body, mut j) in hunts {
        if j.phase == "complete" {
            continue;
        }
        let dt = (w.tick - j.last_tick).max(0).min(1);
        if j.phase != "paused" {
            let permitted = known(w, j.actor, &j.contact)
                .and_then(|c| hunter(w, j.actor, &c).map(|fit| (c, fit)));
            if let Ok((c, fit)) = permitted {
                j.attempted_s += dt;
                // Reports authorize attempts; real equipment must reach the
                // physical body to earn referee-effective work. This private
                // check cannot change admission, own phase or elapsed clock.
                let reached = w
                    .rules
                    .mines
                    .as_ref()
                    .and_then(|s| s.bodies.get(&c.mine_id))
                    .zip(w.entity(j.actor))
                    .is_some_and(|(b, e)| {
                        !b.neutralized
                            && b.case_band <= fit.max_depth_band
                            && e.pos.range_to(b.position) * YD <= fit.range_yd
                    });
                if reached {
                    j.worked_s += dt;
                }
                j.phase = "working".into();
            } else {
                j.phase = "suspended_conditions".into();
            }
        }
        j.last_tick = w.tick;
        if j.worked_s >= j.required_s {
            j.phase = "complete".into();
            if let Some(b) = w.rules.mines.as_mut().unwrap().bodies.get_mut(&body) {
                b.neutralized = true;
            }
            referee(
                w,
                Some(j.actor),
                "13.8.individual_neutralized",
                json!({"mine":body,"contact":j.contact,"d6":j.d6,"worked_s":j.worked_s}),
            );
        }
        w.rules.mines.as_mut().unwrap().hunts.insert(body, j);
    }
    let retired: Vec<_> = w
        .rules
        .mines
        .as_ref()
        .unwrap()
        .manifests
        .iter()
        .filter_map(|(f, ids)| {
            (!ids.is_empty()
                && ids
                    .iter()
                    .all(|id| w.rules.mines.as_ref().unwrap().bodies[id].neutralized))
            .then_some(f.clone())
        })
        .collect();
    for field in retired {
        if w.rules
            .mines
            .as_mut()
            .unwrap()
            .fields
            .remove(&field)
            .is_some()
        {
            referee(
                w,
                None,
                "13.8.complete_manifest_retired",
                json!({"field":field}),
            );
        }
    }
}
fn release(w: &mut World, actor: EntityId, j: &LayJob) -> Result<(), String> {
    let mut f = j.plan.field.clone();
    let stock = w
        .rules
        .mines
        .as_ref()
        .unwrap()
        .stocks
        .get(&actor)
        .and_then(|s| s.get(&f.mine))
        .copied()
        .unwrap_or(0);
    if stock < j.points.len() as u32 {
        return Err("13.3 carried source stock changed before actual release".into());
    }
    let ids: Vec<_> = (0..j.points.len())
        .map(|i| format!("{}:mine:{}", f.id, i + 1))
        .collect();
    let s = w.rules.mines.as_ref().unwrap();
    if s.fields.contains_key(&f.id)
        || s.manifests.contains_key(&f.id)
        || ids.iter().any(|id| s.bodies.contains_key(id))
    {
        return Err("duplicate physical mine/field identity".into());
    }
    f.armed_at = w.tick + arming_seconds(f.case_band, j.plan.arming_s)?;
    f.activation_at = w.tick + j.plan.activation_delay_s;
    if j.fit.kind == "sub" {
        // Physical source reserves are transferred by torpedo executor, never duplicated here.
        if super::super::torpedo2026::mine_inventory(w, actor, &f.mine)? != stock {
            return Err("13.3 shared physical mine total changed".into());
        }
        super::super::torpedo2026::occupy_mine_tubes(
            w,
            actor,
            &j.fit.tube_ids[..j.points.len()],
            &f.mine,
            n("lay", "sub_reload_s") as i64,
        )?;
    }
    let s = w.rules.mines.as_mut().unwrap();
    s.stocks
        .get_mut(&actor)
        .unwrap()
        .insert(f.mine.clone(), stock - j.points.len() as u32);
    for (id, p) in ids.iter().zip(&j.points) {
        s.bodies.insert(
            id.clone(),
            Body {
                id: id.clone(),
                field: f.id.clone(),
                mine: f.mine.clone(),
                owner: f.owner,
                position: *p,
                case_band: f.case_band,
                released_at: w.tick,
                armed_at: f.armed_at,
                activation_at: f.activation_at,
                neutralized: false,
                source: j.plan.source.clone(),
            },
        );
    }
    s.manifests.insert(f.id.clone(), ids.clone());
    s.fields.insert(f.id.clone(), f.clone());
    referee(
        w,
        Some(actor),
        "13.7.complete_row_release",
        json!({"plan":j.plan_id,"field":f.id,"mines":ids,"count":j.points.len(),"worked_s":j.worked_s,"released_at":w.tick,"armed_at":f.armed_at,"activation_at":f.activation_at,"fit_source":j.fit.source,"combat_halved":j.combat_seen}),
    );
    Ok(())
}
pub(super) fn partial(w: &World, field: &str) -> bool {
    w.rules.mines.as_ref().is_some_and(|s| {
        s.manifests
            .get(field)
            .is_some_and(|ids| ids.iter().any(|id| s.bodies[id].neutralized))
    })
}
/// Only own stocks/jobs and source-known report geometry. No truth body/manifest IDs.
pub fn own_view(w: &World, side: Side) -> Option<Value> {
    if !active(w) {
        return None;
    }
    let s = w.rules.mines.as_ref()?;
    let mut units = vec![];
    let mut plans = vec![];
    for e in w
        .entities
        .values()
        .filter(|e| e.side == side && !e.kind.is_projectile())
    {
        let lay=s.lay_jobs.get(&e.id).map(|j|json!({"plan":j.plan_id,"phase":j.phase,"worked_s":j.worked_s,"positions_visited":j.visited.len(),"positions_required":j.points.len()}));
        let hunts:Vec<_>=s.hunts.values().filter(|j|j.actor==e.id).map(|j|json!({"contact":j.contact,"phase":j.phase,"attempted_s":j.attempted_s,"required_s":j.required_s})).collect();
        if let Some(c) = &w.context.mines_2026 {
            for (id, p) in &c.lay_plans {
                if p.unit != e.name
                    || p.field.owner != side
                    || fit_and_plan(w, e.id, id).is_err()
                    || s.fields.contains_key(&p.field.id)
                    || s.manifests.contains_key(&p.field.id)
                    || s.lay_jobs.contains_key(&e.id)
                    || s.gear.get(&e.id).is_some_and(|g| !g.lost)
                    || s.hunts
                        .values()
                        .any(|j| j.actor == e.id && j.phase == "working")
                {
                    continue;
                }
                plans.push(json!({"id":id,"actor":e.id,"source":p.source,"geometry":p.field.area,
                    "mine":p.field.mine,"rows":p.field.rows,"spacing_yd":p.field.spacing_yd,"row_spacing_yd":p.field.row_spacing_yd,
                    "case_band":p.field.case_band,"water_band":p.field.water_band,"activation_delay_s":p.activation_delay_s,
                    "arming_s":arming_seconds(p.field.case_band,p.arming_s).ok()}));
            }
        }
        if s.stocks.contains_key(&e.id)
            || s.charges.contains_key(&e.id)
            || lay.is_some()
            || !hunts.is_empty()
        {
            units.push(json!({"unit":e.id,"stock":s.stocks.get(&e.id),"charges":s.charges.get(&e.id),"lay":lay,"hunts":hunts}));
        }
    }
    let contacts:Vec<_>=s.classified.iter().filter(|c|c.side==side).map(|c|json!({"id":c.id,"position":c.position,"case_band":c.case_band,"classified":c.classified,"source":c.source})).collect();
    let echoes = super::search::own_echoes(s, side);
    (!units.is_empty() || !contacts.is_empty() || !plans.is_empty() || !echoes.is_empty()).then(
        || {
            let mut view = json!({"units":units,"contacts":contacts,"lay_plans":plans});
            if !echoes.is_empty() {
                view["echoes"] = json!(echoes);
            }
            view
        },
    )
}

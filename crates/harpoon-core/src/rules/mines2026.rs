//! Chapter13. Explicit v4; hidden fields never enter player observations.
use crate::{types::PlatformKind, EntityId, EventKind, Rng, Side, Vec2, Visibility, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};
mod lifecycle;
mod search;
pub use lifecycle::{
    own_view, Body, ClassifiedMine, HuntJob, HunterFit, LayFit, LayJob, LayPlan, MineStock,
    PartialPath,
};
pub use search::{
    configure_database, AssumedSearchReceiver, CaseDepth, Echo, EchoReport, SearchFit,
};
pub fn tables() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/mines-2026.json"))
            .expect("mine source tables")
    })
}
fn n(section: &str, key: &str) -> f64 {
    tables()[section][key].as_f64().unwrap()
}
pub fn active(w: &World) -> bool {
    w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub fields: BTreeMap<String, Field>,
    pub positions: BTreeMap<EntityId, Vec2>,
    pub travelled: BTreeMap<String, f64>,
    pub ordinals: BTreeMap<String, u64>,
    pub inside: BTreeSet<String>,
    pub skipped_transits: BTreeSet<String>,
    pub swept: Vec<SweptStrip>,
    pub diagnostics: BTreeSet<String>,
    pub gear: BTreeMap<EntityId, Gear>,
    pub combat: BTreeMap<EntityId, i64>,
    pub pass_sequence: u64,
    pub pending: BTreeMap<String, PendingAttack>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub stocks: BTreeMap<EntityId, BTreeMap<String, u32>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub lay_jobs: BTreeMap<EntityId, LayJob>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub bodies: BTreeMap<String, Body>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub manifests: BTreeMap<String, Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub classified: Vec<ClassifiedMine>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub charges: BTreeMap<EntityId, u32>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub hunts: BTreeMap<String, HuntJob>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub partial_paths: Vec<PartialPath>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub echoes: BTreeMap<String, Echo>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub echo_sequence: BTreeMap<usize, u64>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Configuration {
    #[serde(default)]
    pub fields: Vec<Field>,
    pub source: String,
    #[serde(default)]
    pub capabilities: BTreeMap<String, Capability>,
    #[serde(default)]
    pub permissive: Vec<Rect>,
    #[serde(default)]
    pub deadweight: BTreeMap<String, Deadweight>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub laying_fits: BTreeMap<String, LayFit>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub carried_mines: BTreeMap<String, MineStock>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lay_plans: BTreeMap<String, LayPlan>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hunter_fits: BTreeMap<String, HunterFit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mine_bodies: Vec<Body>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_mine_contacts: Vec<ClassifiedMine>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub field_manifests: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub search_fits: BTreeMap<String, Vec<SearchFit>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub case_depths: BTreeMap<String, CaseDepth>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deadweight {
    pub tonnes: f64,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingAttack {
    pub target: EntityId,
    pub field: String,
    pub tick: i64,
    pub position: Vec2,
    pub d100: u8,
    pub chance: f64,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capability {
    pub source: String,
    pub gear: BTreeSet<String>,
    #[serde(default)]
    pub combined: bool,
    #[serde(default)]
    pub helicopter_stream_s: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gear {
    pub equipment: BTreeSet<String>,
    pub phase: String,
    pub due: i64,
    pub course: f64,
    pub target_course: f64,
    pub start: Vec2,
    pub partner: Option<EntityId>,
    pub helicopter: bool,
    pub lost: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Lay {
        plan_id: String,
    },
    CancelLay,
    Hunt {
        contact_id: String,
    },
    CancelHunt,
    Stream {
        gear: BTreeSet<String>,
        #[serde(default)]
        partner: Option<EntityId>,
    },
    FinishPass,
    Turn {
        course_deg: f64,
    },
    Jettison,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rect {
    pub start: Vec2,
    pub end: Vec2,
    pub width_yd: f64,
}
impl Rect {
    pub fn valid(&self) -> bool {
        [
            self.start.x,
            self.start.y,
            self.end.x,
            self.end.y,
            self.width_yd,
        ]
        .iter()
        .all(|v| v.is_finite())
            && self.width_yd > 0.
            && self.start.range_to(self.end) > 0.
    }
    fn local(&self, p: Vec2) -> (f64, f64) {
        let d = self.end.sub(self.start);
        let len = d.norm();
        let q = p.sub(self.start);
        (
            (q.x * d.x + q.y * d.y) / len,
            (-q.x * d.y + q.y * d.x) / len,
        )
    }
    pub fn contains(&self, p: Vec2) -> bool {
        let (x, y) = self.local(p);
        x >= 0.
            && x <= self.start.range_to(self.end)
            && y.abs() <= self.width_yd / 2025.3718285 / 2.
    }
    pub fn clip(&self, a: Vec2, b: Vec2) -> Option<(f64, f64)> {
        if !self.valid() {
            return None;
        }
        let (ax, ay) = self.local(a);
        let (bx, by) = self.local(b);
        let half = self.width_yd / 2025.3718285 / 2.;
        let mut lo: f64 = 0.;
        let mut hi: f64 = 1.;
        for (p, q) in [
            (-(bx - ax), ax),
            (bx - ax, self.start.range_to(self.end) - ax),
            (-(by - ay), ay + half),
            (by - ay, half - ay),
        ] {
            if p.abs() < 1e-12 {
                if q < 0. {
                    return None;
                }
            } else {
                let t = q / p;
                if p < 0. {
                    lo = lo.max(t);
                } else {
                    hi = hi.min(t);
                }
                if lo > hi {
                    return None;
                }
            }
        }
        Some((lo, hi))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Field {
    pub id: String,
    pub owner: Side,
    pub mine: String,
    pub area: Rect,
    pub rows: u32,
    pub spacing_yd: u16,
    pub row_spacing_yd: u16,
    pub case_band: u8,
    pub water_band: u8,
    pub armed_at: i64,
    #[serde(default)]
    pub activation_at: i64,
    #[serde(default)]
    pub ship_counter: u32,
    #[serde(default)]
    pub crossings: u32,
    #[serde(default)]
    pub optional_damage: bool,
    #[serde(default)]
    pub safe_channels: Vec<Rect>,
    #[serde(default)]
    pub bearing: Option<f64>,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SweptStrip {
    pub area: Rect,
    pub gear: BTreeSet<String>,
    pub half_passes: u32,
    pub tick: i64,
    #[serde(default)]
    pub pass: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Impact {
    pub contact: bool,
    pub moored: bool,
    pub submarine_column: String,
    pub pressure_percent: Option<f64>,
    pub rescue_percent: u8,
    pub rescue_max_band: u8,
}
#[derive(Clone, Debug)]
pub struct Target {
    pub size: usize,
    pub speed: f64,
    pub warship: bool,
    pub dwt: Option<f64>,
    pub depth_band: u8,
    pub submarine: bool,
    pub double_hull: bool,
    pub year: u16,
}
pub fn probability(influence: bool, size: usize, spacing: u16) -> Result<f64, String> {
    if size > 6 {
        return Err("13.5 unknown target size".into());
    }
    let d = &tables()[if influence { "influence" } else { "contact" }];
    let col = d["spacing_yd"]
        .as_array()
        .unwrap()
        .iter()
        .position(|v| v.as_u64() == Some(spacing as u64))
        .ok_or("13.5 spacing has no printed probability column")?;
    Ok(d["probability"][size.min(4)][col].as_f64().unwrap())
}
pub fn layout(
    length: u32,
    spacing: u16,
    stock: u32,
    row_spacing: u16,
) -> Result<(u32, u32, u32), String> {
    if spacing == 0 || length == 0 || length % spacing as u32 != 0 {
        return Err("13.4 fractional mine count needs explicit scenario interpretation".into());
    }
    if row_spacing < (n("lay", "minimum_row_spacing_yd") as u16) {
        return Err("13.4 rows must be at least200yd apart".into());
    }
    let per = length / spacing as u32 + 1;
    let rows = stock / per;
    if rows == 0 {
        return Err("13.4 not enough mines for one row".into());
    }
    Ok((per, rows, (rows + 1) * row_spacing as u32))
}
pub fn sweep_remaining(influence: bool, generation: u8, half_passes: u32) -> Result<f64, String> {
    if !(1..=3).contains(&generation) {
        return Err("13.8.4 generation4 mines require individual hunting".into());
    }
    let rows = tables()[if influence {
        "influence_remaining"
    } else {
        "mechanical_remaining"
    }]
    .as_array()
    .unwrap();
    let passes = (half_passes / 2).min(rows.len() as u32);
    let mut remaining = 1.;
    for row in rows.iter().take(passes as usize) {
        if let Some(v) = row[generation as usize - 1].as_f64() {
            remaining = v;
        }
    }
    Ok(remaining.max(n("sweep", "minimum_remaining")))
}
pub fn mine(id: &str) -> Result<&'static Value, String> {
    tables()["mines"]
        .get(id)
        .ok_or_else(|| format!("AnnexG mine not source-verified: {id}"))
}
pub fn depth_band(m: f64) -> Result<u8, String> {
    if !m.is_finite() || m < 0. {
        return Err("invalid depth".into());
    }
    Ok(tables()["depth_max_m"]
        .as_array()
        .unwrap()
        .iter()
        .position(|v| m <= v.as_f64().unwrap())
        .unwrap_or(12) as u8)
}
pub fn attack_chance(
    field: &Field,
    target: &Target,
    swept_half_passes: u32,
) -> Result<f64, String> {
    if target.size > 6
        || !target.speed.is_finite()
        || target.speed < 0.
        || target.dwt.is_some_and(|v| !v.is_finite() || v < 0.)
    {
        return Err("13.5 invalid target size/speed/dwt".into());
    }
    let spec = mine(&field.mine)?;
    let f = spec["fuzes"].as_array().unwrap();
    let has = |k: &str| f.iter().any(|v| v == k);
    let influence = !has("contact");
    let mut sz = target.size as i32;
    if influence {
        if has("magnetic") && target.warship {
            sz += 1;
        }
        if has("acoustic") && target.speed >= n("attack", "acoustic_large_kts") {
            sz -= 1;
        }
        if has("pressure")
            && target.speed <= n("attack", "pressure_small_kts")
            && target.size < (n("attack", "pressure_no_benefit_from_size") as usize)
        {
            sz += 1;
        }
    }
    let mut p = probability(influence, sz.clamp(0, 6) as usize, field.spacing_yd)?;
    if target.dwt.is_some_and(|v| v >= n("attack", "large_dwt")) {
        p += n("attack", "large_bonus");
    }
    if swept_half_passes > 0 {
        p *= sweep_remaining(
            influence,
            spec["generation"].as_u64().unwrap() as u8,
            swept_half_passes,
        )?;
    }
    Ok((p * 100.).round().min(100.) / 100.)
}
pub fn damage_solution(
    field: &Field,
    target: &Target,
    roll: u8,
    chance: f64,
) -> Result<(f64, Impact), String> {
    let spec = mine(&field.mine)?;
    if field.optional_damage
        && field.case_band == 1
        && (!target.submarine || target.depth_band == 0)
    {
        return Err("13.6 optional damage versus Shallow-to-surface Major limit unresolved; retain original referee die".into());
    }
    if !(1..=100).contains(&roll)
        || !chance.is_finite()
        || chance <= 0.
        || chance > 1.
        || roll as f64 > chance * 100. + 1e-9
    {
        return Err("13.6 requires original successful D100".into());
    }
    let contact = spec["fuzes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "contact");
    let tier = if !contact && field.optional_damage {
        let fraction = roll as f64 / (chance * 100.);
        tables()["attack"]["optional_damage_thresholds"]
            .as_array()
            .unwrap()
            .iter()
            .position(|v| fraction <= v.as_f64().unwrap() + 1e-12)
            .unwrap()
    } else if !contact && field.case_band == 1 && (!target.submarine || target.depth_band == 0) {
        2
    } else {
        0
    };
    let dp = spec["damage"][tier]
        .as_f64()
        .ok_or("13.6 required AnnexG damage tier unverified")?;
    let chance = if target.submarine {
        if contact || field.optional_damage {
            if target.year == 0 {
                return Err("8.5.3 submarine in-service year unknown".into());
            }
            if target.size > 3 {
                return Err("8.5.3 submarine size unverified".into());
            }
            let p = tables()["hull"][if target.double_hull {
                "contact_double_percent"
            } else {
                "contact_single_percent"
            }][target.size]
                .as_f64()
                .unwrap()
                + if target.year < 1955 {
                    n("hull", "pre1955_add_percent")
                } else {
                    0.
                };
            Some(
                (p * if contact {
                    n("hull", "contact_multiplier")
                } else {
                    1.
                })
                .min(100.),
            )
        } else {
            Some(n(
                "hull",
                if target.size <= 1 {
                    "influence_large_medium"
                } else {
                    "influence_small_vsmall"
                },
            ))
        }
    } else {
        None
    };
    Ok((
        dp,
        Impact {
            contact,
            moored: spec["kind"] == "moored",
            submarine_column: if tier == 3 { "sub_minor" } else { "sub_major" }.into(),
            pressure_percent: chance,
            rescue_percent: n("hull", "shallow_rescue_percent") as u8,
            rescue_max_band: 1,
        },
    ))
}
pub fn eligible(field: &Field, target: &Target) -> Result<bool, String> {
    let spec = mine(&field.mine)?;
    if target.submarine && target.depth_band > 0 {
        return Ok(target.depth_band == field.case_band);
    }
    if field.case_band > 1 {
        return Ok(false);
    }
    if spec["fuzes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "contact")
    {
        Ok(field.case_band == 0)
    } else {
        Ok(field.case_band == 0 || target.size <= 2)
    }
}
pub fn arming_seconds(case_band: u8, explicit: Option<i64>) -> Result<i64, String> {
    if let Some(s) = explicit {
        if s < 0 {
            return Err("negative authored arming delay".into());
        }
        return Ok(s);
    }
    let key = match case_band {
        1 => "shallow",
        2 => "int_i",
        _ => return Err("13.7 other arming bands need explicit scenario source".into()),
    };
    Ok(tables()["lay"]["arming_s"][key].as_i64().unwrap())
}
fn referee(w: &mut World, id: Option<EntityId>, rule: &str, detail: Value) {
    w.emit(
        EventKind::RulesEvent {
            domain: "mines2026".into(),
            rule: rule.into(),
            side: None,
            unit: id,
            contact: None,
            detail: detail.to_string(),
        },
        Visibility::Referee,
    );
}
fn diagnostic(w: &mut World, id: EntityId, key: String, reason: String) {
    if w.rules.mines.as_mut().unwrap().diagnostics.insert(key) {
        referee(
            w,
            Some(id),
            "adjudication_blocked",
            json!({"reason":reason}),
        );
    }
}
pub fn initialize(w: &mut World) -> Result<(), String> {
    if !active(w) {
        return Ok(());
    }
    let config = w.context.mines_2026.clone().unwrap_or_default();
    let mut s = State::default();
    for f in config.fields.clone() {
        if f.id.is_empty()
            || f.source.is_empty()
            || !f.area.valid()
            || f.rows == 0
            || f.row_spacing_yd < 200
            || f.water_band < f.case_band
            || f.safe_channels.iter().any(|r| !r.valid())
        {
            return Err("13.4 invalid/unsourced minefield geometry or depth".into());
        }
        let spec = mine(&f.mine)?;
        if !matches!(spec["kind"].as_str(), Some("bottom" | "moored")) {
            return Err("13.6 special mobile/drifting/propelled mine requires its dedicated movement and attack procedure; cannot use regular mine rows".into());
        }
        if spec["max_case_band"]
            .as_u64()
            .is_some_and(|m| f.case_band > m as u8)
        {
            return Err("13.4 mine case exceeds AnnexG limit".into());
        }
        if spec["max_case_band"].is_null() && (f.case_band > 0 || f.water_band > 0) {
            return Err("13.4 AnnexG has one depth column but moored case/anchor limits require separate confirmation".into());
        }
        if matches!(spec["kind"].as_str(), Some("bottom" | "mobile")) && f.water_band != f.case_band
        {
            return Err("13.4 bottom mine case must be at actual bottom depth".into());
        }
        if (f.area.width_yd - (f.rows + 1) as f64 * f.row_spacing_yd as f64).abs() > 1e-7 {
            return Err("13.4 danger depth must equal(rows+1)×row spacing".into());
        }
        let along = f.area.start.range_to(f.area.end) * 2025.3718285 / f.spacing_yd as f64;
        if (along - along.round()).abs() > 1e-6 {
            return Err("13.4 fractional mines per line not source-defined".into());
        }
        probability(
            !spec["fuzes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "contact"),
            0,
            f.spacing_yd,
        )?;
        if s.fields.insert(f.id.clone(), f).is_some() {
            return Err("duplicate minefield identity".into());
        }
    }
    for e in w.entities.values() {
        s.positions.insert(e.id, e.pos);
    }
    lifecycle::initialize(w, &config, &mut s)?;
    search::initialize(w, &config)?;
    w.rules.mines = Some(s);
    Ok(())
}
pub fn target(w: &World, id: EntityId) -> Result<Target, String> {
    let e = w.entity(id).ok_or("target missing")?;
    let p = e
        .rule_data
        .as_ref()
        .or_else(|| super::damage_runtime::platform(&e.class))
        .ok_or("13.5 target size/role lacks source")?;
    let size = match p.size.as_str() {
        "A" => 0,
        "B" => 1,
        "C" => 2,
        "D" => 3,
        "E" => 4,
        "F" => 5,
        "G" => 6,
        _ => return Err("13.5 source size unknown".into()),
    };
    if p.source.is_empty() {
        return Err("13.5 target source required".into());
    }
    Ok(Target {
        size,
        speed: e.speed_kts,
        warship: !matches!(p.role.as_str(), "merchant" | "auxiliary"),
        dwt: w
            .context
            .mines_2026
            .as_ref()
            .and_then(|c| c.deadweight.get(&e.name))
            .map(|d| d.tonnes),
        depth_band: depth_band(e.depth_m)?,
        submarine: e.kind == PlatformKind::Sub,
        double_hull: p.double_hull,
        year: p.year,
    })
}
pub fn field_segments(f: &Field, a: Vec2, b: Vec2) -> Vec<(f64, f64)> {
    let Some((lo, hi)) = f.area.clip(a, b) else {
        return vec![];
    };
    let mut safe: Vec<(f64, f64)> = f
        .safe_channels
        .iter()
        .filter_map(|c| c.clip(a, b))
        .filter_map(|(x, y)| {
            let l = x.max(lo);
            let h = y.min(hi);
            (h > l).then_some((l, h))
        })
        .collect();
    safe.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut segments = vec![];
    let mut cursor = lo;
    for (l, h) in safe {
        if l > cursor {
            segments.push((cursor, l));
        }
        cursor = cursor.max(h);
    }
    if cursor < hi {
        segments.push((cursor, hi));
    }
    segments
}
pub fn field_distance(f: &Field, a: Vec2, b: Vec2) -> f64 {
    field_segments(f, a, b)
        .iter()
        .map(|(lo, hi)| hi - lo)
        .sum::<f64>()
        * a.range_to(b)
        * 2025.3718285
}
pub fn attack_positions(
    f: &Field,
    a: Vec2,
    b: Vec2,
    previous: f64,
    interval: f64,
) -> (Vec<Vec2>, f64) {
    let scale = a.range_to(b) * 2025.3718285;
    if scale < 1e-12 {
        return (vec![], previous);
    }
    let mut carry = previous;
    let mut positions = vec![];
    for (lo, hi) in field_segments(f, a, b) {
        let mut cursor = lo;
        while carry + (hi - cursor) * scale + 1e-8 >= interval {
            let next = cursor + (interval - carry) / scale;
            positions.push(a.add(b.sub(a).scaled(next.clamp(lo, hi))));
            cursor = next;
            carry = 0.;
        }
        carry += (hi - cursor) * scale;
    }
    (positions, carry)
}
fn swept_passes(w: &World, f: &Field, p: Vec2) -> u32 {
    let spec = mine(&f.mine).unwrap();
    let contact = spec["fuzes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "contact");
    w.rules
        .mines
        .as_ref()
        .unwrap()
        .swept
        .iter()
        .filter(|s| {
            s.area.contains(p)
                && if contact || spec["kind"] == "moored" {
                    s.gear.contains("oropesa") || s.gear.contains("a_sweep")
                } else {
                    spec["fuzes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|v| s.gear.contains(v.as_str().unwrap()))
                }
        })
        .map(|s| s.half_passes)
        .sum()
}
pub fn step(w: &mut World) {
    if !active(w) || w.rules.mines.is_none() {
        return;
    }
    gear_step(w);
    lifecycle::step(w);
    search::step(w);
    let units: Vec<_> = w
        .entities
        .values()
        .filter(|e| e.alive && matches!(e.kind, PlatformKind::Ship | PlatformKind::Sub))
        .map(|e| (e.id, e.pos))
        .collect();
    let fields: Vec<_> = w
        .rules
        .mines
        .as_ref()
        .unwrap()
        .fields
        .keys()
        .cloned()
        .collect();
    for (id, pos) in units {
        let old = w
            .rules
            .mines
            .as_mut()
            .unwrap()
            .positions
            .insert(id, pos)
            .unwrap_or(pos);
        for fid in &fields {
            let f = w.rules.mines.as_ref().unwrap().fields[fid].clone();
            let key = format!("{}:{}", fid, id.0);
            let at = f.area.contains(pos);
            let was = w.rules.mines.as_ref().unwrap().inside.contains(&key);
            let distance = field_distance(&f, old, pos);
            if distance > 0. && !was {
                let s = w.rules.mines.as_mut().unwrap();
                let live = s.fields.get_mut(fid).unwrap();
                live.crossings += 1;
                if live.crossings <= live.ship_counter {
                    s.skipped_transits.insert(key.clone());
                } else {
                    s.skipped_transits.remove(&key);
                }
            }
            let skip = w
                .rules
                .mines
                .as_ref()
                .unwrap()
                .skipped_transits
                .contains(&key);
            if at {
                w.rules.mines.as_mut().unwrap().inside.insert(key.clone());
            } else {
                let s = w.rules.mines.as_mut().unwrap();
                s.inside.remove(&key);
                s.skipped_transits.remove(&key);
            }
            if w.tick < f.armed_at.max(f.activation_at) || skip || distance <= 0. {
                continue;
            }
            if lifecycle::partial(w, fid) {
                w.rules
                    .mines
                    .as_mut()
                    .unwrap()
                    .partial_paths
                    .push(PartialPath {
                        field: fid.clone(),
                        target: id,
                        tick: w.tick,
                        from: old,
                        to: pos,
                        danger_distance_yd: distance,
                    });
                diagnostic(w, id, format!("partial:{key}"), "13.8.4 partially neutralized explicit manifest has no source density-to-Ph mapping; preserve actual crossing for referee adjudication".into());
                continue;
            }
            let mut t = match target(w, id) {
                Ok(t) => t,
                Err(e) => {
                    diagnostic(w, id, key, e);
                    continue;
                }
            };
            if w.rules
                .mines
                .as_ref()
                .unwrap()
                .gear
                .get(&id)
                .is_some_and(|g| !g.lost && g.phase == "active")
            {
                t.size = 2;
            }
            if !eligible(&f, &t).unwrap_or(false) {
                continue;
            }
            if mine(&f.mine).is_ok_and(|s| {
                s["fuzes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v == "contact")
            }) && w
                .entity(id)
                .and_then(|e| {
                    e.rule_data
                        .as_ref()
                        .or_else(|| super::damage_runtime::platform(&e.class))
                })
                .is_some_and(|p| p.tps > 0.)
            {
                diagnostic(w,id,key,"13.6 contact-mine TPS vs14.1.7 torpedo-only TPS scope conflict; automatic mine hit/damage blocked, source interpretation required".into());
                continue;
            }
            let interval = (f.area.width_yd / f.rows as f64 / n("attack", "round_distance_yd"))
                .round()
                * n("attack", "round_distance_yd");
            if interval <= 0. {
                diagnostic(w, id, key, "13.5 zero attack interval".into());
                continue;
            }
            let prior = w
                .rules
                .mines
                .as_ref()
                .unwrap()
                .travelled
                .get(&key)
                .copied()
                .unwrap_or(0.);
            let (positions, carry) = attack_positions(&f, old, pos, prior, interval);
            w.rules
                .mines
                .as_mut()
                .unwrap()
                .travelled
                .insert(key.clone(), carry);
            for point in positions {
                let ordinal = w
                    .rules
                    .mines
                    .as_mut()
                    .unwrap()
                    .ordinals
                    .entry(key.clone())
                    .or_default();
                *ordinal += 1;
                let identity = format!("mine:{key}:{ordinal}");
                let half = swept_passes(w, &f, point);
                let chance = match attack_chance(&f, &t, half) {
                    Ok(p) => p,
                    Err(e) => {
                        diagnostic(w, id, key.clone(), e);
                        break;
                    }
                };
                let mut r = w.rng.derive(Rng::hash_tag(0, id.0, &identity));
                let roll = (r.next_f64() * 100.).floor() as u8 + 1;
                referee(
                    w,
                    Some(id),
                    "13.5.attack",
                    json!({"identity":identity,"field":fid,"roll_d100":roll,"chance":chance,"distance_yd":interval,"swept_half_passes":half}),
                );
                if roll as f64 <= chance * 100. + 1e-9 {
                    match damage_solution(&f, &t, roll, chance) {
                        Ok((dp, impact)) => super::damage_runtime::queue_mine(
                            w, id, dp, impact, f.bearing, &identity,
                        ),
                        Err(e) => {
                            w.rules.mines.as_mut().unwrap().pending.insert(
                                identity.clone(),
                                PendingAttack {
                                    target: id,
                                    field: fid.clone(),
                                    tick: w.tick,
                                    position: point,
                                    d100: roll,
                                    chance,
                                    reason: e.clone(),
                                },
                            );
                            diagnostic(w, id, key.clone(), e);
                        }
                    }
                }
            }
        }
    }
}
fn capability(w: &World, id: EntityId) -> Result<Capability, String> {
    let e = w
        .entity(id)
        .filter(|e| e.alive)
        .ok_or("minesweeper missing")?;
    let c = w
        .context
        .mines_2026
        .as_ref()
        .and_then(|c| c.capabilities.get(&e.name))
        .ok_or("13.8 source equipment capability not bound")?;
    if c.source.trim().is_empty() {
        return Err("13.8 capability needs source or explicit scenario assumption".into());
    }
    Ok(c.clone())
}
fn permissive(w: &World, id: EntityId) -> bool {
    w.entity(id).is_some_and(|e| {
        w.context
            .mines_2026
            .as_ref()
            .is_some_and(|c| c.permissive.iter().any(|a| a.contains(e.pos)))
    }) && w
        .rules
        .mines
        .as_ref()
        .and_then(|s| s.combat.get(&id))
        .is_none_or(|t| w.tick - *t >= 180)
}
pub fn record_combat(w: &mut World, event: &EventKind) {
    if !active(w) {
        return;
    }
    let id = match event {
        EventKind::WeaponsRelease { shooter, .. }
        | EventKind::GunAction { shooter, .. }
        | EventKind::SamShot { shooter, .. } => Some(*shooter),
        EventKind::Impact { unit, .. } => Some(*unit),
        _ => None,
    };
    if let Some((id, s)) = id.zip(w.rules.mines.as_mut()) {
        s.combat.insert(id, w.tick);
    }
}
fn tow_speed(g: &Gear) -> f64 {
    if g.helicopter {
        n("sweep", "helicopter_max_kts")
    } else {
        g.equipment
            .iter()
            .map(|k| {
                tables()["sweep"]["gear"][k]["tow_max_kts"]
                    .as_f64()
                    .unwrap()
            })
            .fold(f64::INFINITY, f64::min)
    }
}
/// Speed/course orders consumed by real movement, including automatic routes.
pub fn navigation_limits(w: &World, id: EntityId) -> (Option<f64>, Option<f64>) {
    if !active(w) {
        return (None, None);
    }
    let Some(g) = w
        .rules
        .mines
        .as_ref()
        .and_then(|s| s.gear.get(&id))
        .filter(|g| !g.lost)
    else {
        return (None, None);
    };
    let speed = if g.phase == "deploying" {
        5.
    } else {
        tow_speed(g)
    };
    let course = if matches!(g.phase.as_str(), "short_stay" | "restream" | "active") {
        Some(g.course)
    } else if g.phase == "turning" {
        Some(g.target_course)
    } else {
        None
    };
    (Some(speed), course)
}
pub fn apply(w: &mut World, id: EntityId, action: &Action) -> Result<(), String> {
    if !active(w) {
        return Err("source mine warfare requires v4".into());
    }
    if matches!(
        action,
        Action::Lay { .. } | Action::CancelLay | Action::Hunt { .. } | Action::CancelHunt
    ) {
        return lifecycle::apply(w, id, action);
    }
    let e = w
        .entity(id)
        .filter(|e| e.alive)
        .ok_or("minesweeper missing")?
        .clone();
    let cap = capability(w, id)?;
    match action {
        Action::Stream { gear, partner } => {
            if gear.is_empty() || !gear.is_subset(&cap.gear) || gear.len() > 1 && !cap.combined {
                return Err("13.8 unverified sweep equipment or simultaneous combination".into());
            }
            if w.rules.mines.as_ref().unwrap().gear.contains_key(&id) {
                return Err(
                    "sweep gear already streamed, deploying or lost; no guessed spare set".into(),
                );
            }
            if e.speed_kts > 5. {
                return Err("13.8 streaming requires<=5kt".into());
            }
            let helicopter = e.kind == PlatformKind::Aircraft;
            if helicopter
                && (!permissive(w, id) || e.altitude_m > n("sweep", "helicopter_max_altitude_m"))
            {
                return Err(
                    "13.8.2 helicopter requires authored permissive environment and VLow".into(),
                );
            }
            let mut due = 0;
            for k in gear {
                let d = &tables()["sweep"]["gear"][k];
                due = due.max(d["deploy_s"].as_i64().ok_or("13.8 unknown gear")?);
                if k == "a_sweep" {
                    let p=partner.and_then(|id|w.entity(id)).filter(|p|p.alive&&p.side==e.side&&p.speed_kts<=5.&&p.pos.range_to(e.pos)*2025.3718285<=d["pair_max_yd"].as_f64().unwrap()).ok_or("13.8 A-sweep needs a second equipped friendly ship at<=5kt within250yd")?;
                    if !capability(w, p.id)?.gear.contains(k) {
                        return Err("partner has no verified A-sweep fit".into());
                    }
                }
            }
            if helicopter {
                due=cap.helicopter_stream_s.ok_or("13.8.2 helicopter streaming duration needs explicit scenario procedure; no ship timing guessed")?;
                if due < 0 {
                    return Err("invalid helicopter streaming duration".into());
                }
            }
            w.rules.mines.as_mut().unwrap().gear.insert(
                id,
                Gear {
                    equipment: gear.clone(),
                    phase: "deploying".into(),
                    due: w.tick + due,
                    course: e.course_deg,
                    target_course: e.course_deg,
                    start: e.pos,
                    partner: *partner,
                    helicopter,
                    lost: false,
                },
            );
        }
        Action::FinishPass => {
            finish_pass(w, id)?;
        }
        Action::Turn { course_deg } => {
            if !course_deg.is_finite() {
                return Err("invalid course".into());
            }
            let g = w
                .rules
                .mines
                .as_ref()
                .unwrap()
                .gear
                .get(&id)
                .filter(|g| !g.lost && g.phase == "active")
                .ok_or("13.8 sweep must be streamed before turning")?
                .clone();
            if g.equipment.contains("oropesa")
                && crate::geom::angle_diff(g.course, *course_deg).abs() > 45. + 1e-9
            {
                return Err("13.8 Oropesa turn limited to one45degree interval".into());
            }
            if e.pos.range_to(g.start) > 1e-8 {
                finish_pass(w, id)?;
            }
            let mechanical = g.equipment.contains("oropesa");
            let due = w.tick
                + if mechanical {
                    tables()["sweep"]["gear"]["oropesa"]["short_stay_s"]
                        .as_i64()
                        .unwrap()
                } else {
                    0
                };
            let g = w.rules.mines.as_mut().unwrap().gear.get_mut(&id).unwrap();
            g.phase = if mechanical { "short_stay" } else { "turning" }.into();
            g.due = due;
            g.target_course = crate::geom::normalize_deg(*course_deg);
        }
        Action::Jettison => {
            let g = w
                .rules
                .mines
                .as_mut()
                .unwrap()
                .gear
                .get_mut(&id)
                .ok_or("no deployed gear")?;
            g.lost = true;
            g.phase = "jettisoned".into();
        }
        _ => unreachable!("lifecycle actions dispatched before sweep capability gate"),
    }
    referee(
        w,
        Some(id),
        "13.8.order",
        json!({"action":action,"source":cap.source}),
    );
    Ok(())
}
fn finish_pass(w: &mut World, id: EntityId) -> Result<(), String> {
    let g = w
        .rules
        .mines
        .as_ref()
        .unwrap()
        .gear
        .get(&id)
        .filter(|g| !g.lost && g.phase == "active")
        .ok_or("13.8 gear not streamed")?
        .clone();
    let e = w.entity(id).ok_or("sweeper missing")?;
    if e.pos.range_to(g.start) < 1e-8 {
        return Err("13.8 stationary sweeping is not a pass".into());
    }
    if e.speed_kts > tow_speed(&g)
        || g.helicopter
            && (!permissive(w, id) || e.altitude_m > n("sweep", "helicopter_max_altitude_m"))
    {
        return Err("13.8 unsafe sweep receives no pass credit".into());
    }
    let end = e.pos;
    let width = g
        .equipment
        .iter()
        .map(|k| tables()["sweep"]["gear"][k]["width_yd"].as_f64().unwrap())
        .fold(f64::INFINITY, f64::min);
    let s = w.rules.mines.as_mut().unwrap();
    s.pass_sequence += 1;
    let pass = format!("{}:{}", id.0, s.pass_sequence);
    s.swept.push(SweptStrip {
        area: Rect {
            start: g.start,
            end,
            width_yd: width,
        },
        gear: g.equipment,
        half_passes: if g.helicopter { 1 } else { 2 },
        tick: w.tick,
        pass: pass.clone(),
    });
    s.gear.get_mut(&id).unwrap().start = end;
    referee(
        w,
        Some(id),
        "13.8.pass",
        json!({"pass":pass,"width_yd":width,"half_passes":if g.helicopter{1}else{2}}),
    );
    Ok(())
}
fn gear_step(w: &mut World) {
    let ids: Vec<_> = w
        .rules
        .mines
        .as_ref()
        .unwrap()
        .gear
        .keys()
        .copied()
        .collect();
    for id in ids {
        let Some(e) = w.entity(id).filter(|e| e.alive).cloned() else {
            continue;
        };
        let g = w.rules.mines.as_ref().unwrap().gear[&id].clone();
        if g.lost {
            continue;
        }
        if g.phase == "deploying" && e.speed_kts > 5. {
            let cap = capability(w, id).unwrap();
            let duration = if g.helicopter {
                cap.helicopter_stream_s.unwrap()
            } else {
                g.equipment
                    .iter()
                    .map(|k| tables()["sweep"]["gear"][k]["deploy_s"].as_i64().unwrap())
                    .max()
                    .unwrap()
            };
            let q = w.rules.mines.as_mut().unwrap().gear.get_mut(&id).unwrap();
            q.due = w.tick + duration;
            continue;
        }
        if g.helicopter
            && (!permissive(w, id) || e.altitude_m > n("sweep", "helicopter_max_altitude_m"))
        {
            w.rules
                .mines
                .as_mut()
                .unwrap()
                .gear
                .get_mut(&id)
                .unwrap()
                .start = e.pos;
            continue;
        }
        let next = if g.phase == "deploying" && w.tick >= g.due {
            Some(("active", w.tick, e.course_deg))
        } else if g.phase == "short_stay" && w.tick >= g.due {
            Some(("turning", w.tick, g.target_course))
        } else if g.phase == "turning"
            && crate::geom::angle_diff(e.course_deg, g.target_course).abs() < 1e-6
        {
            if !g.equipment.contains("oropesa") {
                Some(("active", w.tick, g.target_course))
            } else {
                Some((
                    "restream",
                    w.tick
                        + tables()["sweep"]["gear"]["oropesa"]["restream_s"]
                            .as_i64()
                            .unwrap(),
                    g.target_course,
                ))
            }
        } else if g.phase == "restream" && w.tick >= g.due {
            Some(("active", w.tick, g.course))
        } else {
            None
        };
        if let Some((phase, due, course)) = next {
            let q = w.rules.mines.as_mut().unwrap().gear.get_mut(&id).unwrap();
            q.phase = phase.into();
            q.due = due;
            q.course = course;
            q.start = e.pos;
            referee(
                w,
                Some(id),
                "13.8.gear_phase",
                json!({"phase":phase,"due":due}),
            );
        }
    }
}

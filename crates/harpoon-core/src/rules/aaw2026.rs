//! Harpoon V 2026 §8.1; opt-in v4. Table data and annex fits are separate.
//! Missing metadata and unprinted rounding are errors, never old Pk fallbacks.
use crate::{
    db::{WeaponDef, WeaponKind},
    geom::angle_diff,
    types::PlatformKind,
    EntityId, Rng, World,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/aaw-2026.json"))
            .expect("verified AAW tables")
    })
}
pub fn active(w: &World) -> bool {
    w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full
}

/// Explicit fictional incoming targets for scenario/algorithm verification.
/// Real annex weapon identifiers cannot receive missing values through this map.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    #[serde(default)]
    pub incoming_profiles: BTreeMap<String, IncomingProfile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingProfile {
    pub signature: usize,
    /// Must be present, even when explicitly unknown. Never defaults to zero.
    #[serde(deserialize_with = "required_incoming_mr")]
    pub maneuver_rating: Option<f64>,
    pub maneuvering: bool,
    pub terminal_maneuvers: bool,
    pub high_diving: bool,
    pub sprint_vehicle: bool,
    pub source: String,
}

fn required_incoming_mr<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f64>, D::Error> {
    Option::<f64>::deserialize(deserializer)
}

pub fn validate_configuration(configuration: &Configuration) -> Result<(), String> {
    for (key, profile) in &configuration.incoming_profiles {
        if !key
            .strip_prefix("scenario_assumption_")
            .is_some_and(|suffix| !suffix.is_empty())
        {
            return Err("8.1 authored incoming key must use scenario_assumption_ with a fictional name; real annex IDs cannot be overridden".into());
        }
        if profile.source != "scenario_assumption" {
            return Err(
                "8.1 fictional incoming profile requires source=scenario_assumption".into(),
            );
        }
        if profile.signature > 4 {
            return Err("8.1 authored incoming signature outside source classes".into());
        }
        if profile
            .maneuver_rating
            .is_some_and(|mr| !mr.is_finite() || mr < 0.0)
        {
            return Err(
                "8.1 authored incoming MR must be finite and nonnegative or explicit null".into(),
            );
        }
    }
    Ok(())
}

fn configured_incoming_profile<'a>(
    w: &'a World,
    weapon: &str,
) -> Result<Option<&'a IncomingProfile>, String> {
    if !active(w) {
        return Ok(None);
    }
    let Some(configuration) = w.context.aaw_2026.as_ref() else {
        return Ok(None);
    };
    validate_configuration(configuration)?;
    Ok(configuration.incoming_profiles.get(weapon))
}

/// Radar signature only; this does not supply physical size or target MR.
pub fn incoming_radar_signature(w: &World, weapon: &str) -> Option<usize> {
    configured_incoming_profile(w, weapon)
        .ok()
        .flatten()
        .map(|profile| profile.signature)
}

fn n(v: &Value) -> f64 {
    v.as_f64().expect("numeric source cell")
}
fn finite(v: f64, label: &str) -> Result<(), String> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(format!("8.1 unknown {label}"))
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Band {
    Point,
    Short,
    Medium1,
    Medium2,
    Long1,
    Long2,
    VeryLong,
    Extreme,
}
impl Band {
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn family(self) -> usize {
        match self {
            Self::Point => 0,
            Self::Short => 1,
            Self::Medium1 | Self::Medium2 => 2,
            Self::Long1 | Self::Long2 => 3,
            Self::VeryLong => 4,
            Self::Extreme => 5,
        }
    }
}
pub fn band(range: f64) -> Result<Band, String> {
    finite(range, "range")?;
    if range < 0.0 {
        return Err("negative range".into());
    }
    Ok(if range <= 4.0 {
        Band::Point
    } else if range <= 15.0 {
        Band::Short
    } else if range <= 30.0 {
        Band::Medium1
    } else if range <= 45.0 {
        Band::Medium2
    } else if range <= 60.0 {
        Band::Long1
    } else if range <= 90.0 {
        Band::Long2
    } else if range <= 135.0 {
        Band::VeryLong
    } else {
        Band::Extreme
    })
}
pub fn speed_column(speed: f64) -> Result<usize, String> {
    finite(speed, "speed")?;
    if speed < 0.0 {
        return Err("negative speed".into());
    }
    Ok(if speed <= 750.0 {
        0
    } else if speed <= 1525.0 {
        1
    } else if speed <= 2300.0 {
        2
    } else if speed <= 3075.0 {
        3
    } else if speed <= 4100.0 {
        4
    } else if speed <= 5125.0 {
        5
    } else {
        6
    })
}
fn speed_modifier(table: &str, speed: f64) -> Result<f64, String> {
    finite(speed, "speed")?;
    if speed < 0.0 {
        return Err("negative speed".into());
    }
    let rows = tables()[table].as_array().unwrap();
    Ok(n(&rows
        .iter()
        .find(|r| r[0].is_null() || speed <= n(&r[0]))
        .unwrap()[1]))
}
pub fn speed_ata(speed: f64) -> Result<f64, String> {
    speed_modifier("missile_speed_modifiers", speed)
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Bearing {
    Closing,
    Divergent,
    Crossing,
}
pub fn bearing_rate(range: f64, speed: f64, aspect: f64) -> Result<Bearing, String> {
    finite(aspect, "aspect")?;
    if !(0.0..=180.0).contains(&aspect) {
        return Err("invalid aspect".into());
    }
    let a = if aspect > 90.0 { aspect - 90.0 } else { aspect };
    let row = band(range)?.index().saturating_sub(speed_column(speed)?);
    let limits = &tables()["crossing_max_angles"][row];
    Ok(if a <= n(&limits[0]) {
        Bearing::Closing
    } else if a <= n(&limits[1]) {
        Bearing::Divergent
    } else {
        Bearing::Crossing
    })
}
pub fn bearing_modifier(b: Bearing) -> f64 {
    n(&tables()["target_modifiers"]["bearing"][format!("{b:?}")])
}
pub fn f_pole(range: f64, speed: f64, closing: bool, maneuvering: bool) -> Result<f64, String> {
    finite(range, "SAM range")?;
    finite(speed, "target speed")?;
    if range < 0.0 || speed < 0.0 {
        return Err("invalid F-pole input".into());
    }
    Ok(range
        * if closing && !maneuvering {
            if speed >= 2001.0 {
                2.0
            } else if speed >= 501.0 {
                1.5
            } else {
                1.0
            }
        } else {
            1.0
        })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Budget {
    pub count: [u8; 6],
    pub source_cell: String,
    pub table_band: Band,
}
impl Budget {
    pub fn allows(&self, b: Band, used: &[u8; 6]) -> bool {
        used[b.family()] < self.count[b.family()]
    }
}
fn parse_budget(s: &str, b: Band) -> Result<Budget, String> {
    let mut count = [0u8; 6];
    for p in s.split('-') {
        let ch = p.chars().last().ok_or("empty intercept token")?;
        let i = match ch {
            'P' => 0,
            'S' => 1,
            'M' => 2,
            'L' => 3,
            'V' => 4,
            'E' => 5,
            _ => return Err("invalid intercept token".into()),
        };
        count[i] = if p.len() == 1 {
            1
        } else {
            p[..p.len() - 1]
                .parse()
                .map_err(|_| "invalid intercept count")?
        };
    }
    Ok(Budget {
        count,
        source_cell: s.into(),
        table_band: b,
    })
}
fn cs_row(generation: usize, automatic: bool) -> Result<usize, String> {
    Ok(match (generation, automatic) {
        (2, _) => 0,
        (3, _) => 1,
        (4, _) => 2,
        (5 | 6, false) => 3,
        (5, true) => 4,
        (6, true) => 5,
        _ => return Err("8.1.1 no printed intercept row for this combat system".into()),
    })
}
pub fn sam_channels(
    generation: usize,
    automatic: bool,
    guidance: &str,
) -> Result<(u8, u8), String> {
    tables()["sam_salvo_summary"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r["gen"].as_u64() == Some(generation as u64)
                && (r["control"] == "any"
                    || r["control"] == if automatic { "automatic" } else { "human" })
                && r["guidance"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|x| x == guidance)
        })
        .map(|r| {
            (
                r["channels"].as_u64().unwrap() as u8,
                r["missiles"].as_u64().unwrap() as u8,
            )
        })
        .ok_or("6.2.1 this guidance/system combination has no printed channel row".into())
}
/// Full printed three pages (four altitude groups); -- drops to first legal shorter column.
/// `extra` is a player's explicit selection of Long1 or Medium2 for a >=3000kt SAM.
pub fn intercept_budget(
    generation: usize,
    automatic: bool,
    altitude_m: f64,
    target_speed: f64,
    lookup_range: f64,
    sam_speed: f64,
    min_range: f64,
    receding: bool,
    extra: Option<Band>,
) -> Result<Budget, String> {
    finite(altitude_m, "altitude")?;
    let row = cs_row(generation, automatic)?;
    let col = speed_column(target_speed)?;
    let a = super::operations::altitude_band(altitude_m);
    let group = match a {
        0 | 1 => "vlow",
        2 => "low",
        3 => "medium",
        4 | 5 => "high",
        _ => return Err("8.1.1 no printed RHigh SAM intercept table".into()),
    };
    let g = &tables()["intercept"][group];
    let mut idx = band(lookup_range)?.index().max(1);
    let mut result = loop {
        let key = idx.to_string();
        let v = &g[&key][row][col];
        if let Some(s) = v.as_str() {
            let b = [
                Band::Point,
                Band::Short,
                Band::Medium1,
                Band::Medium2,
                Band::Long1,
                Band::Long2,
                Band::VeryLong,
                Band::Extreme,
            ][idx];
            break parse_budget(s, b)?;
        }
        if idx == 1 {
            return Err("8.1.1 kinematically impossible (printed --)".into());
        }
        idx -= 1;
    };
    if let Some(e) = extra {
        if sam_speed < 3000.0 || !matches!(e, Band::Long1 | Band::Medium2) {
            return Err("8.1.1 invalid extra high-speed engagement selection".into());
        }
        if e.index() <= result.table_band.index() {
            result.count[e.family()] += 1;
        } else {
            return Err("8.1.1 extra engagement is outside this interception window".into());
        }
    }
    if min_range > 2.5 {
        result.count[0] = 0;
    }
    if receding {
        // Only the unambiguous printed example is enabled; do not invent the multi-shot sub-band shift.
        if result.source_cell == "M-2S-P" && result.table_band == Band::Medium1 {
            result.count = [0, 1, 1, 0, 0, 0];
        } else {
            return Err(
                "8.1.6 receding shot shift unresolved outside printed M-2S-P example".into(),
            );
        }
    }
    Ok(result)
}
pub fn missile_probability(index: f64, missiles: u8) -> Result<f64, String> {
    finite(index, "missile index")?;
    if !(1..=2).contains(&missiles) {
        return Err("8.1.1 source table supports one or two missiles per target".into());
    }
    let key = index * 2.0;
    if (key - key.round()).abs() > 1e-7 {
        return Err(
            "8.1 missile index lies between printed half-step rows; rounding unresolved".into(),
        );
    }
    tables()["missile_attack"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| (n(&r[0]) - index).abs() < 1e-7)
        .map(|r| n(&r[missiles as usize]) / 100.0)
        .ok_or("8.1 missile index outside printed table; no extrapolation".into())
}
pub fn gun_probability(strength: f64) -> Result<f64, String> {
    finite(strength, "AA strength")?;
    if strength <= 0.0 {
        return Ok(0.02);
    }
    if strength >= 10.6 {
        return Ok(0.90);
    }
    if (strength * 10.0 - (strength * 10.0).round()).abs() > 1e-7 {
        return Err("8.1 AA strength between printed tenth rows; rounding unresolved".into());
    }
    tables()["gun_attack"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| (n(&r[0]) - strength).abs() < 1e-7)
        .map(|r| n(&r[1]))
        .ok_or("8.1 missing AA strength row".into())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Countermeasures {
    pub seeker_generation: usize,
    pub jammer_generation: Option<usize>,
    pub decoy_generation: Option<usize>,
}
pub fn countermeasure_modifier(c: &Countermeasures, maneuvering: bool) -> Result<f64, String> {
    if !(1..=5).contains(&c.seeker_generation) {
        return Err("unknown seeker generation".into());
    }
    let g = match (c.jammer_generation, c.decoy_generation) {
        (None, None) => return Ok(0.0),
        (Some(g), None) => (0, g),
        (None, Some(g)) => (1, g),
        (Some(j), Some(d)) if j == d => (2, j),
        _ => return Err("mixed jammer/decoy generations need source combination procedure".into()),
    };
    if !(1..=4).contains(&g.1) {
        return Err("unknown countermeasure generation".into());
    }
    Ok(
        n(&tables()["countermeasures"][c.seeker_generation - 1][g.0][g.1 - 1])
            / if maneuvering { 1.0 } else { 2.0 },
    )
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MissileAttack {
    pub ata: f64,
    pub maneuver_rating: f64,
    pub maneuvering: bool,
    pub speed_kts: f64,
    pub signature: usize,
    pub combat_generation: usize,
    pub automatic: bool,
    pub target_is_missile: bool,
    pub range_nm: f64,
    pub bearing: Bearing,
    pub terminal_maneuvers: bool,
    pub sea_skimming: bool,
    pub sea_capability: u8,
    pub noe: bool,
    pub manpads_or_salh: bool,
    pub high_diving: bool,
    pub land_clutter: bool,
    pub radar_guided: bool,
    pub countermeasures: Option<Countermeasures>,
    pub hoj: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Solution {
    pub value: f64,
    pub probability: f64,
    pub modifiers: BTreeMap<String, f64>,
}
pub fn missile_solution(a: &MissileAttack, missiles: u8) -> Result<Solution, String> {
    if a.signature > 4 || a.sea_capability > 2 {
        return Err("unknown signature/sea capability".into());
    }
    finite(a.ata, "ATA")?;
    finite(a.maneuver_rating, "MR")?;
    let mut m = BTreeMap::new();
    let key = match a.combat_generation {
        2 | 3 | 4 => a.combat_generation.to_string(),
        5 | 6 => format!(
            "{}{}",
            a.combat_generation,
            if a.automatic { 'a' } else { 'h' }
        ),
        _ => return Err("unknown CS modifier".into()),
    };
    m.insert(
        "maneuver_rating".into(),
        if a.maneuvering {
            -a.maneuver_rating
        } else {
            0.0
        },
    );
    m.insert("speed".into(), speed_ata(a.speed_kts)?);
    m.insert(
        "signature".into(),
        n(&tables()["signature_ata"][a.signature]),
    );
    m.insert(
        "combat_system".into(),
        n(&tables()["combat_system"][&key][if a.target_is_missile { 0 } else { 1 }]),
    );
    m.insert(
        "range".into(),
        n(&tables()["range_ata"][band(a.range_nm)?.index()]),
    );
    m.insert("bearing".into(), bearing_modifier(a.bearing));
    m.insert(
        "terminal_maneuvers".into(),
        if a.terminal_maneuvers {
            n(&tables()["target_modifiers"]["terminal_maneuvers"])
        } else {
            0.0
        },
    );
    m.insert(
        "sea_skimmer".into(),
        if a.sea_skimming {
            n(&tables()["target_modifiers"]["sea_capability"][a.sea_capability as usize])
        } else {
            0.0
        },
    );
    m.insert(
        "NOE".into(),
        if a.noe && !a.manpads_or_salh {
            n(&tables()["target_modifiers"]["NOE"])
        } else {
            0.0
        },
    );
    m.insert(
        "high_diving".into(),
        if a.high_diving && a.combat_generation <= 3 && band(a.range_nm)? <= Band::Short {
            n(&tables()["target_modifiers"]["high_diving"])
        } else {
            0.0
        },
    );
    m.insert(
        "land_clutter".into(),
        if a.land_clutter && a.radar_guided {
            n(&tables()["target_modifiers"]["land_clutter"])
        } else {
            0.0
        },
    );
    m.insert(
        "countermeasures".into(),
        if a.hoj {
            0.0
        } else {
            a.countermeasures
                .as_ref()
                .map(|c| countermeasure_modifier(c, a.maneuvering))
                .transpose()?
                .unwrap_or(0.0)
        },
    );
    let value = a.ata / if a.hoj { 2.0 } else { 1.0 } + m.values().sum::<f64>();
    Ok(Solution {
        value,
        probability: missile_probability(value, missiles)?,
        modifiers: m,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GunAttack {
    pub base_strength: f64,
    pub combat_generation: usize,
    pub speed_kts: f64,
    pub signature: usize,
    pub altitude_m: f64,
    pub noe: bool,
    pub sskim_capable: bool,
    pub terminal_maneuvers: bool,
    pub non_maneuvering: bool,
    pub non_maneuvering_entire_pd: bool,
    pub caliber_mm: f64,
    pub bearing: Bearing,
    pub high_diving: bool,
    pub radar_guided: bool,
    pub land_clutter: bool,
    pub local_control_of_directed: bool,
}
pub fn gun_solution(a: &GunAttack) -> Result<Solution, String> {
    if a.signature > 4 || !(1..=6).contains(&a.combat_generation) {
        return Err("unknown gun target or CS metadata".into());
    }
    finite(a.base_strength, "AA strength")?;
    finite(a.altitude_m, "altitude")?;
    finite(a.caliber_mm, "gun caliber")?;
    if a.altitude_m < 0.0 || a.caliber_mm <= 0.0 {
        return Err("invalid gun geometry/caliber".into());
    }
    let d = &tables()["gun_modifiers"];
    let mut m = BTreeMap::new();
    m.insert(
        "combat_system".into(),
        n(&d["combat_system"][a.combat_generation - 1]),
    );
    m.insert(
        "speed".into(),
        speed_modifier("gun_speed_modifiers", a.speed_kts)?,
    );
    m.insert(
        "signature".into(),
        n(&tables()["signature_gun"][a.signature]),
    );
    m.insert(
        "altitude".into(),
        if a.noe {
            n(&d["NOE"])
        } else if a.altitude_m <= 20.0 && !a.sskim_capable {
            n(&d["SSkim_not_capable"])
        } else if a.altitude_m > 7500.0 {
            n(&d["High"])
        } else {
            0.0
        },
    );
    m.insert(
        "maneuvers".into(),
        if a.terminal_maneuvers {
            n(&d["terminal_maneuvers"])
        } else if a.non_maneuvering
            && (a.caliber_mm < n(&d["large_caliber_non_maneuvering_threshold_mm"])
                || a.non_maneuvering_entire_pd)
        {
            n(&d["non_maneuvering"])
        } else {
            0.0
        },
    );
    m.insert("bearing".into(), bearing_modifier(a.bearing));
    m.insert(
        "high_diving".into(),
        if a.high_diving {
            n(&d["high_diving"])
        } else {
            0.0
        },
    );
    m.insert(
        "land_clutter".into(),
        if a.radar_guided && a.land_clutter {
            n(&d["land_clutter"])
        } else {
            0.0
        },
    );
    let value = a.base_strength
        * if a.local_control_of_directed {
            n(&d["directed_local_factor"])
        } else {
            1.0
        }
        + m.values().sum::<f64>();
    Ok(Solution {
        value,
        probability: gun_probability(value)?,
        modifiers: m,
    })
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub aam: super::aam2026::State,
    pub firecontrol: super::firecontrol2026::State,
    pub surface: super::surface2026::State,
    pub engagements: BTreeMap<String, Engagement>,
    pub increments: BTreeMap<String, BurstWindow>,
    pub outcomes: BTreeMap<String, Outcome>,
    pub shots: BTreeMap<EntityId, Packet>,
    pub diagnostics: BTreeSet<String>,
    pub gun_passes: BTreeSet<String>,
    pub turn_away: BTreeMap<EntityId, i64>,
    pub gun_total: BTreeMap<String, u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Engagement {
    pub budget: Budget,
    pub used: [u8; 6],
    pub first_tick: i64,
    pub initial_course: f64,
    pub initial_range: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BurstWindow {
    pub start: i64,
    pub used: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outcome {
    pub identity: String,
    pub d100: u8,
    pub probability: f64,
    pub hit: bool,
    pub consumed_projectiles: Vec<EntityId>,
    pub solution: Solution,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Permit {
    pub key: String,
    pub channel: usize,
    pub shooter: EntityId,
    pub target: EntityId,
    pub weapon: String,
    pub tick: i64,
    pub band: Band,
    pub budget: Budget,
    pub initial_course: f64,
    pub initial_range: f64,
    pub attack: MissileAttack,
    pub missiles: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Packet {
    pub identity: String,
    pub permit: Permit,
    pub members: Vec<EntityId>,
}
/// Commit only after every projectile in the packet actually exists; no phantom opportunity consumption.
pub fn commit(state: &mut State, p: Permit, projectiles: &[EntityId]) -> Result<(), String> {
    if projectiles.len() != p.missiles as usize || projectiles.is_empty() {
        return Err("actual projectile count differs from authorized SAM packet".into());
    }
    let e = state
        .engagements
        .entry(p.key.clone())
        .or_insert_with(|| Engagement {
            budget: p.budget.clone(),
            used: [0; 6],
            first_tick: p.tick,
            initial_course: p.initial_course,
            initial_range: p.initial_range,
        });
    if !e.budget.allows(p.band, &e.used) {
        return Err("SAM firing opportunity already consumed".into());
    }
    e.used[p.band.family()] += 1;
    let identity = format!(
        "aaw:{}:{}:{}:{}",
        p.shooter.0, p.weapon, p.target.0, projectiles[0].0
    );
    let packet = Packet {
        identity,
        permit: p,
        members: projectiles.to_vec(),
    };
    for id in projectiles {
        state.shots.insert(*id, packet.clone());
    }
    Ok(())
}
/// One combined D100 per packet, source one/two missile columns. Cached across saves.
pub fn terminal(w: &World, state: &mut State, projectile: EntityId) -> Result<Outcome, String> {
    let p = state
        .shots
        .get(&projectile)
        .ok_or("SAM projectile has no committed source packet")?
        .clone();
    if let Some(o) = state.outcomes.get(&p.identity) {
        return Ok(o.clone());
    }
    let mut solution = missile_solution(&p.permit.attack, p.members.len() as u8)?;
    if state
        .turn_away
        .get(&projectile)
        .is_some_and(|since| w.tick - *since >= 180)
    {
        let shooter = w
            .entity(p.permit.shooter)
            .ok_or("shooter metadata unavailable")?;
        let profile = weapon_profile(&shooter.class, &p.permit.weapon)?;
        if p.permit.initial_range > n(&profile["range_nm"]) {
            solution.probability = 0.0;
            solution
                .modifiers
                .insert("forced_miss_f_pole_lost".into(), 1.0);
        }
    }
    let mut rng = w.rng.derive(Rng::hash_tag(
        p.permit.tick,
        p.permit.shooter.0,
        &p.identity,
    ));
    let d100 = 1 + (rng.next_f64() * 100.0) as u8;
    let outcome = Outcome {
        identity: p.identity.clone(),
        d100,
        probability: solution.probability,
        hit: d100 as f64 <= solution.probability * 100.0,
        consumed_projectiles: p.members.clone(),
        solution,
    };
    state.outcomes.insert(p.identity, outcome.clone());
    Ok(outcome)
}
/// Per independent physical mount; first shot fixes a local 30s window.
pub fn consume_burst(state: &mut State, key: &str, now: i64, limit: u8) -> Result<u8, String> {
    let w = state.increments.entry(key.into()).or_insert(BurstWindow {
        start: now,
        used: 0,
    });
    if now >= w.start + 30 {
        w.start = now;
        w.used = 0;
    }
    if now < w.start {
        return Err("AAW clock moved backwards".into());
    }
    if w.used >= limit {
        return Err("8.1.4 burst allowance used for this 30-second increment".into());
    }
    w.used += 1;
    Ok(w.used)
}
pub fn can_burst(state: &State, key: &str, now: i64, limit: u8) -> bool {
    state
        .increments
        .get(key)
        .is_none_or(|w| now >= w.start + 30 || w.used < limit)
}
pub fn friendly_restriction(w: &World, shooter: EntityId, target: EntityId) -> bool {
    let Some(s) = w.entity(shooter) else {
        return true;
    };
    let Some(t) = w.entity(target) else {
        return true;
    };
    let Some(friend) = t
        .projectile
        .as_ref()
        .and_then(|p| p.target)
        .and_then(|i| w.entity(i))
    else {
        return false;
    };
    friend.alive
        && friend.kind == PlatformKind::Ship
        && friend.side == s.side
        && friend.id != shooter
        && t.speed_kts > 0.0
        && t.pos.range_to(friend.pos) / t.speed_kts * 3600.0 <= 30.0
}
pub fn weapon_profile(class: &str, weapon: &str) -> Result<&'static Value, String> {
    tables()["weapon_profiles"]
        .get(format!("{class}:{weapon}"))
        .or_else(|| tables()["weapon_profiles"].get(weapon))
        .ok_or_else(|| {
            format!("8.1 source-verified ATA/guidance/envelope/fit missing for {class}:{weapon}")
        })
}
pub fn world_target(w: &World, target: EntityId) -> Result<(Value, usize), String> {
    let t = w.entity(target).ok_or("target absent")?;
    let key = t
        .projectile
        .as_ref()
        .map(|p| p.weapon.as_str())
        .unwrap_or(&t.class);
    let p = if let Some(profile) = tables()["target_profiles"].get(key) {
        profile.clone()
    } else if let Some(profile) = super::surface2026::antiship::incoming_aaw_profile(w, target)? {
        profile.clone()
    } else if let Some(profile) = configured_incoming_profile(w, key)? {
        serde_json::to_value(profile).map_err(|e| format!("8.1 authored incoming profile: {e}"))?
    } else {
        return Err(
            "8.1 target signature/terminal behavior not verified; referee data required".into(),
        );
    };
    let sig = p["signature"].as_u64().ok_or("unknown source signature")? as usize;
    Ok((p, sig))
}
fn land_clutter(w: &World, s: EntityId, t: EntityId) -> Result<bool, String> {
    if w.context.terrain.is_some() && w.context.radar_land_clutter.is_none() {
        return Err("8.1 radar AA needs authored/verified land geometry classification".into());
    }
    let a = w.entity(s).unwrap();
    let b = w.entity(t).unwrap();
    Ok(w.context.radar_land_clutter.as_ref().is_some_and(|r| {
        r.iter()
            .any(|r| r.affects(a.pos, a.pos.bearing_to(b.pos), a.pos.range_to(b.pos) + 5.0))
    }))
}
pub fn world_prepare(
    w: &World,
    state: &State,
    shooter: EntityId,
    target: EntityId,
    def: &WeaponDef,
    missiles: u8,
) -> Result<Permit, String> {
    let s = w
        .entity(shooter)
        .filter(|s| s.alive)
        .ok_or("shooter unavailable")?;
    let t = w
        .entity(target)
        .filter(|t| t.alive)
        .ok_or("target unavailable")?;
    if def.kind != WeaponKind::Sam {
        return Err("8.1 source packet requires SAM".into());
    }
    if friendly_restriction(w, shooter, target) {
        return Err("8.1.8 no AA fire/guide during final 30s to another friendly ship".into());
    }
    let p = weapon_profile(&s.class, &def.id)?;
    let (target_data, signature, countermeasures) = if t.kind == PlatformKind::Aircraft {
        let seeker = p["seeker_generation"]
            .as_u64()
            .ok_or("SAM seeker generation missing")? as usize;
        let (mr, maneuvering, sig, cm) = super::aam2026::target_data(w, target, shooter, seeker)?;
        (
            json!({"maneuver_rating":mr,"maneuvering":maneuvering,"terminal_maneuvers":false,"high_diving":false}),
            sig,
            Some(cm),
        )
    } else {
        let (td, sig) = world_target(w, target)?;
        (td, sig, None)
    };
    // R-059: an official Kh-22N example has no MR deduction, but does not
    // establish a default MR for every real missile. Unknown is not numeric 0.
    // Guns use world_target independently and do not require this SAM input.
    let maneuver_rating = target_data["maneuver_rating"]
        .as_f64()
        .ok_or("8.1 target MR unresolved (R-059); strict SAM source adjudication pending")?;
    finite(maneuver_rating, "target MR")?;
    let cs = super::operations::tables()["aaw"]["combat_systems"]
        .get(&s.class)
        .ok_or("8.1 combat system source fit unavailable")?;
    let gen = cs[0].as_u64().unwrap() as usize;
    let automatic = cs[1] == 1;
    let remote = super::communications2026::received_air_solution(w, shooter, target)?;
    let target_position = remote.as_ref().map(|r| r.position).unwrap_or(t.pos);
    let target_altitude = remote
        .as_ref()
        .and_then(|r| r.altitude_m)
        .unwrap_or(t.altitude_m);
    let range = s.pos.range_to(target_position);
    let aspect = angle_diff(t.course_deg, target_position.bearing_to(s.pos)).abs();
    let receding = aspect > 90.0;
    let br = bearing_rate(range, t.speed_kts, aspect)?;
    let terminal_maneuvers = target_data["terminal_maneuvers"]
        .as_bool()
        .ok_or("unknown terminal maneuver capability")?;
    let max = f_pole(
        n(&p["range_nm"]),
        t.speed_kts,
        !receding && br == Bearing::Closing,
        terminal_maneuvers,
    )?;
    if range > max || range < n(&p["min_range_nm"]) {
        return Err("8.1 SAM outside verified source interception envelope".into());
    }
    let band = band(range)?;
    if band == Band::Point && n(&p["min_range_nm"]) > 2.5 {
        return Err("8.1 SAM minimum range forbids point-defense shot".into());
    }
    if target_altitude > n(&p["max_alt_m"]) || target_altitude < n(&p["min_alt_m"]) {
        return Err("8.1 target outside source SAM altitude envelope".into());
    }
    let director = p["director_sensor"]
        .as_str()
        .ok_or("6.2.1 exact source fire-control sensor missing")?;
    if !s.sensors.iter().any(|m| m.sensor == director)
        || crate::sensors::sensor_damaged(s, director)
    {
        return Err("6.2.1 source fire-control director unavailable/damaged".into());
    }
    let (per_director, per_packet) = sam_channels(
        gen,
        automatic,
        p["guidance"]
            .as_str()
            .ok_or("source SAM guidance unknown")?,
    )?;
    let directors = p["directors"]
        .as_u64()
        .ok_or("source physical director count unknown")? as usize;
    if missiles > per_packet {
        return Err("6.2.1 packet exceeds source guidance missile allowance".into());
    }
    let capacity = p["channels"]
        .as_u64()
        .ok_or("6.2.1 source director/channel fit missing")? as usize;
    if capacity != directors * per_director as usize {
        return Err("6.2.1 profile channel count disagrees with source salvo table".into());
    }
    let channel = (0..capacity)
        .find(|channel| {
            !state.shots.values().any(|packet| {
                packet.permit.shooter == shooter
                    && packet.permit.weapon == def.id
                    && packet.permit.channel == *channel
                    && packet
                        .members
                        .iter()
                        .any(|id| w.entity(*id).is_some_and(|e| e.alive))
            })
        })
        .ok_or("6.2.1 every source channel is guiding a live packet")?;
    let key = format!("{}:{}:{}:{}", shooter.0, def.id, target.0, channel);
    let budget = state
        .engagements
        .get(&key)
        .map(|e| e.budget.clone())
        .map(Ok)
        .unwrap_or_else(|| {
            intercept_budget(
                gen,
                automatic,
                target_altitude,
                t.speed_kts,
                range.min(n(&p["range_nm"])),
                n(&p["speed_kts"]),
                n(&p["min_range_nm"]),
                receding,
                None,
            )
        })?;
    let used = state
        .engagements
        .get(&key)
        .map(|e| e.used)
        .unwrap_or([0; 6]);
    if !budget.allows(band, &used) {
        return Err("8.1 no remaining intercept opportunity in this range family".into());
    }
    let attack = MissileAttack {
        ata: n(&p["ata"]),
        maneuver_rating,
        maneuvering: target_data["maneuvering"]
            .as_bool()
            .ok_or("unknown maneuvering state")?,
        speed_kts: t.speed_kts,
        signature,
        combat_generation: gen,
        automatic,
        target_is_missile: t.kind == PlatformKind::Missile,
        range_nm: range,
        bearing: br,
        terminal_maneuvers,
        sea_skimming: target_altitude <= 20.0,
        sea_capability: p["sea_capability"].as_u64().unwrap() as u8,
        noe: false,
        manpads_or_salh: false,
        high_diving: target_data["high_diving"].as_bool().unwrap_or(false),
        land_clutter: land_clutter(w, shooter, target)?,
        radar_guided: p["radar_guided"] == true,
        countermeasures,
        hoj: false,
    };
    missile_solution(&attack, missiles)?;
    Ok(Permit {
        key,
        channel,
        shooter,
        target,
        weapon: def.id.clone(),
        tick: w.tick,
        band,
        budget,
        initial_course: t.course_deg,
        initial_range: range,
        attack,
        missiles,
    })
}
pub fn visual_sam_legal(
    range: f64,
    visually_detected: bool,
    receding: bool,
    rear_aspect_only: bool,
) -> bool {
    visually_detected && range <= 4.0 && range >= 0.0 && (!rear_aspect_only || receding)
}
pub fn small_arms_legal(range_nm: f64, altitude_m: f64, embarked_infantry: bool) -> bool {
    embarked_infantry && range_nm <= 914.4 / 1852.0 && altitude_m <= 2000.0
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DazzleEffect {
    AbortAndReturn { climb_low: bool },
    OpticalSensorsLost,
    OpticalMissileMiss,
}
pub fn dazzler_solution(
    connected: bool,
    fog: bool,
    precipitation: bool,
    manned: bool,
    uav: bool,
    optical_missile: bool,
    altitude_m: f64,
) -> Result<(f64, u8, DazzleEffect), String> {
    if fog || precipitation {
        return Err("8.1.9 LDS blocked by fog or precipitation".into());
    }
    let effect = if manned {
        DazzleEffect::AbortAndReturn {
            climb_low: altitude_m <= 60.0,
        }
    } else if uav {
        DazzleEffect::OpticalSensorsLost
    } else if optical_missile {
        DazzleEffect::OpticalMissileMiss
    } else {
        return Err("8.1.9 target has no vulnerable optical guidance/crew".into());
    };
    Ok((
        if connected { 0.60 } else { 0.30 },
        if connected { 2 } else { 1 },
        effect,
    ))
}
pub fn trace(w: &mut World, unit: EntityId, rule: &str, detail: Value) {
    let side = w.entity(unit).map(|e| e.side);
    // These records contain referee target modifiers, probabilities and raw
    // rolls. Keep the full audit record without projecting it to either side.
    let visibility = if active(w)
        && matches!(
            rule,
            "8.1.3.source_gun" | "8.1.1.packet_terminal" | "9.5.packet_terminal"
        ) {
        crate::Visibility::Referee
    } else {
        side.map(crate::Visibility::Side)
            .unwrap_or(crate::Visibility::Referee)
    };
    w.emit(
        crate::EventKind::RulesEvent {
            domain: "aaw2026".into(),
            rule: rule.into(),
            side,
            unit: Some(unit),
            contact: None,
            detail: detail.to_string(),
        },
        visibility,
    );
}
pub fn blocked(w: &mut World, unit: EntityId, rule: &str, reason: &str) {
    let key = format!("{}:{rule}:{reason}", unit.0);
    if w.rules
        .aaw
        .as_mut()
        .is_some_and(|s| s.diagnostics.insert(key))
    {
        trace(w, unit, rule, json!({"status":"blocked","reason":reason}));
    }
}

pub fn verified_weapon_def(class: &str, def: &WeaponDef) -> Result<WeaponDef, String> {
    let p = weapon_profile(class, &def.id)?;
    let mut out = def.clone();
    out.range_nm = n(&p["range_nm"]);
    out.min_range_nm = n(&p["min_range_nm"]);
    out.speed_kts = n(&p["speed_kts"]);
    Ok(out)
}
/// Source §2.2.1/diagram2-2. Slash splits physical mounts; ampersand unions arcs.
pub fn arc_allows(arc: &str, relative_bearing: f64) -> Result<bool, String> {
    finite(relative_bearing, "relative bearing")?;
    let b = relative_bearing.rem_euclid(360.0);
    let mut hit = false;
    for a in arc.split('&') {
        let spans = tables()["arcs"]
            .get(a)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("unknown source firing arc {a}"))?;
        hit |= spans.iter().any(|r| b >= n(&r[0]) && b <= n(&r[1]));
    }
    Ok(hit)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GunPermit {
    pub key: String,
    pub shooter: EntityId,
    pub target: EntityId,
    pub weapon: String,
    pub mount_index: usize,
    pub tick: i64,
    pub limit: u8,
    pub ammunition_engagements: Option<u32>,
    pub autonomous: bool,
    pub solution: Solution,
}
pub fn world_gun_solution(
    w: &World,
    state: &State,
    shooter: EntityId,
    target: EntityId,
    def: &WeaponDef,
    mount_index: usize,
) -> Result<GunPermit, String> {
    if !super::ground2026::operational(w, shooter) {
        return Err("11.1 source facility weapon function unavailable".into());
    }
    let s = w
        .entity(shooter)
        .filter(|e| e.alive)
        .ok_or("gun shooter absent")?;
    let t = w
        .entity(target)
        .filter(|e| e.alive)
        .ok_or("AA target absent")?;
    let p = tables()["gun_fits"]
        .get(format!("{}:{}", s.class, def.id))
        .ok_or("8.1.3 AA strength/director/firing arcs not source-verified")?;
    let range = s.pos.range_to(t.pos);
    if range > 4.0 {
        return Err("8.1.3 AA gun only fires in PD band".into());
    }
    if range > n(&p["range_kyd"]) * 914.4 / 1852.0 || t.altitude_m > n(&p["max_alt_m"]) {
        return Err("8.1.3 source gun air envelope exceeded".into());
    }
    if friendly_restriction(w, shooter, target) {
        return Err("8.1.8 friendly vessel firing restriction".into());
    }
    let aspect = angle_diff(t.course_deg, t.pos.bearing_to(s.pos)).abs();
    if aspect > 90.0 {
        return Err("8.1.6 AA guns cannot engage receding target".into());
    }
    let arcs = p["arcs"].as_array().ok_or("gun arcs missing")?;
    let arc = arcs
        .get(mount_index)
        .and_then(Value::as_str)
        .ok_or("gun mount index absent from source fit")?;
    if !arc_allows(arc, s.pos.bearing_to(t.pos) - s.course_deg)? {
        return Err("2.2.1 physical AA mount arc does not bear".into());
    }
    let auto = p["autonomous"] == true;
    if !auto {
        let director = p["director_sensor"]
            .as_str()
            .ok_or("6.2.2 AA director identity not source-bound")?;
        let available = s.sensors.iter().any(|m| m.sensor == director)
            && !crate::sensors::sensor_damaged(s, director)
            && s.systems.radar > 0.05
            && crate::sensors::radar_emitting(s);
        if !available {
            return Err(if p["local_control_allowed"] == false {
                "6.2.3 source gun has No Local Control and its director is unavailable"
            } else {
                "6.2.3 directed gun requires an explicit source local-control mode"
            }
            .into());
        }
        let director_arc = p["director_arc"]
            .as_str()
            .ok_or("6.2.2 AA director arc not source-bound")?;
        if !arc_allows(director_arc, s.pos.bearing_to(t.pos) - s.course_deg)? {
            return Err("6.2.2 AA director arc does not bear".into());
        }
    }
    let key = format!("{}:{}:{}", shooter.0, def.id, mount_index);
    let limit = if auto { 2 } else { 1 };
    let total = state.gun_total.get(&key).copied().unwrap_or(0);
    let ammo = p["ammunition_engagements"].as_u64().map(|x| x as u32);
    if ammo.is_some_and(|cap| total >= cap) {
        return Err("8.1 source gun mount engagement ammunition exhausted".into());
    }
    if !can_burst(state, &key, w.tick, limit) {
        return Err("8.1.4 mount bursts exhausted during this 30s increment".into());
    }
    if !auto && state.gun_passes.contains(&format!("{key}:{}", target.0)) {
        return Err(
            "8.1.3 non-autonomous AA mount already attacked this target's PD transit".into(),
        );
    }
    let (td, signature) = if t.kind == PlatformKind::Aircraft {
        (
            json!({"terminal_maneuvers":false,"maneuvering":super::aircombat2026::maneuvering(w,target),"high_diving":false}),
            super::aircombat2026::target_signature(w, target, shooter)?,
        )
    } else {
        let (td, sig) = world_target(w, target)?;
        (td, sig)
    };
    let cs = tables()["combat_system_bindings"]
        .get(&s.class)
        .ok_or("gun combat system not source-verified")?;
    let attack = GunAttack {
        base_strength: n(&p["aa_strength"]),
        combat_generation: cs[0].as_u64().unwrap() as usize,
        speed_kts: t.speed_kts,
        signature,
        altitude_m: t.altitude_m,
        noe: false,
        sskim_capable: p["sskim_capable"] == true,
        terminal_maneuvers: td["terminal_maneuvers"] == true,
        non_maneuvering: td["maneuvering"] == false,
        non_maneuvering_entire_pd: td["maneuvering"] == false
            && !td["terminal_maneuvers"].as_bool().unwrap_or(true),
        caliber_mm: n(&p["caliber_mm"]),
        bearing: bearing_rate(range, t.speed_kts, aspect)?,
        high_diving: td["high_diving"] == true,
        radar_guided: true,
        land_clutter: land_clutter(w, shooter, target)?,
        local_control_of_directed: false,
    };
    Ok(GunPermit {
        key,
        shooter,
        target,
        weapon: def.id.clone(),
        mount_index,
        tick: w.tick,
        limit,
        ammunition_engagements: ammo,
        autonomous: auto,
        solution: gun_solution(&attack)?,
    })
}
pub fn commit_gun(state: &mut State, permit: &GunPermit) -> Result<u8, String> {
    if permit
        .ammunition_engagements
        .is_some_and(|cap| state.gun_total.get(&permit.key).copied().unwrap_or(0) >= cap)
    {
        return Err("source gun ammunition exhausted".into());
    }
    let burst = consume_burst(state, &permit.key, permit.tick, permit.limit)?;
    *state.gun_total.entry(permit.key.clone()).or_default() += 1;
    if !permit.autonomous {
        state
            .gun_passes
            .insert(format!("{}:{}", permit.key, permit.target.0));
    }
    Ok(burst)
}
pub fn gun_roll(w: &World, state: &mut State, permit: &GunPermit) -> Result<Outcome, String> {
    let burst = commit_gun(state, permit)?;
    let count = state.gun_total[&permit.key];
    let identity = format!("aaw:gun:{}:{}:{burst}:{count}", permit.key, permit.tick);
    let mut r = w
        .rng
        .derive(Rng::hash_tag(permit.tick, permit.shooter.0, &identity));
    let d100 = 1 + (r.next_f64() * 100.0) as u8;
    let outcome = Outcome {
        identity: identity.clone(),
        d100,
        probability: permit.solution.probability,
        hit: d100 as f64 <= permit.solution.probability * 100.0,
        consumed_projectiles: vec![],
        solution: permit.solution.clone(),
    };
    state.outcomes.insert(identity, outcome.clone());
    Ok(outcome)
}
/// Advance turn-away duration continuously, instead of checking one instantaneous course at impact.
pub fn update(w: &mut World) {
    if !active(w) {
        return;
    }
    let Some(mut state) = w.rules.aaw.take() else {
        return;
    };
    state
        .shots
        .retain(|id, _| w.entity(*id).is_some_and(|e| e.alive));
    state.turn_away.retain(|id, _| state.shots.contains_key(id));
    state.gun_passes.retain(|key| {
        let ids: Vec<_> = key.split(':').collect();
        let pair = ids
            .first()
            .and_then(|s| s.parse::<u32>().ok())
            .zip(ids.last().and_then(|s| s.parse::<u32>().ok()));
        pair.is_some_and(|(s, t)| {
            w.entity(EntityId(s))
                .zip(w.entity(EntityId(t)))
                .is_some_and(|(s, t)| t.alive && s.pos.range_to(t.pos) <= 4.)
        })
    });
    for (id, packet) in &state.shots {
        if state.outcomes.contains_key(&packet.identity) {
            continue;
        }
        let away = w
            .entity(packet.permit.target)
            .is_some_and(|t| angle_diff(t.course_deg, packet.permit.initial_course).abs() >= 90.0);
        if away {
            state.turn_away.entry(*id).or_insert(w.tick);
        } else {
            state.turn_away.remove(id);
        }
    }
    w.rules.aaw = Some(state);
}

/// Only the new rule identity calls this. Existing saves retain their embedded DB and v3 fits.
pub fn configure_database(db: &mut crate::Database) {
    super::aam2026::configure_database(db);
    super::aircombat2026::configure_database(db);
    if let Some(mut sensor) = db.sensors.get("radar_air_long").cloned() {
        sensor.id = "us_spy3_fc".into();
        sensor.name = "AN/SPY-3多功能火控雷达（3D主模式）".into();
        sensor.air_range_nm = 150.;
        sensor.missile_range_nm = 48.;
        sensor.surface_range_nm = 22.5;
        db.sensors.insert(sensor.id.clone(), sensor);
    }
    if let Some(ford) = db.platforms.get_mut("us_ford") {
        if !ford.sensors.iter().any(|s| s == "us_spy3_fc") {
            ford.sensors.push("us_spy3_fc".into());
        }
    }
    let essm = WeaponDef {
        id: "us_essm_i".into(),
        name: "RIM-162 ESSM Blk I / 改进型海麻雀I型".into(),
        kind: WeaponKind::Sam,
        targets: crate::db::Targets {
            air: true,
            missile: true,
            ..Default::default()
        },
        range_nm: 18.0,
        min_range_nm: 0.5,
        speed_kts: 2250.0,
        enable_range_nm: 0.0,
        pk: 0.0,
        salvo: 2,
        reload_s: 0.0,
        damage: 25.0,
        guidance: crate::db::Guidance::HomingAir,
        ir: false,
        directors: 4,
        rule_data: None,
    };
    db.weapons.insert(essm.id.clone(), essm);
    if let Some(mut gun) = db.weapons.get("gun_5in").cloned() {
        gun.id = "us_mk45_mod4".into();
        gun.name = "Mk45 Mod4 127毫米/62舰炮".into();
        gun.targets.air = true;
        gun.targets.missile = true;
        db.weapons.insert(gun.id.clone(), gun);
    }
    if let Some(burke) = db.platforms.get_mut("us_burke_2a") {
        for m in &mut burke.weapons {
            if m.weapon == "gun_5in" {
                m.weapon = "us_mk45_mod4".into();
            }
        }
    }
    if let Some(ford) = db.platforms.get_mut("us_ford") {
        for m in &mut ford.weapons {
            if matches!(m.weapon.as_str(), "us_essm" | "sam_medium") {
                m.weapon = "us_essm_i".into();
                m.mounts = 2;
                m.ammo = 8;
            }
            if m.weapon == "ciws_30mm" {
                m.mounts = 3;
                m.ammo = 7;
            }
        }
    }
}
fn retire(w: &mut World, id: EntityId) {
    super::surface2026::antiship::on_intercept(w, id);
    let now = w.tick;
    if let Some(e) = w.entity_mut(id) {
        e.alive = false;
        e.hp = 0.0;
        e.last_emit_tick = now - 1000;
    }
}
/// The caller supplies a contact-backed target. Source admission and mutation are one transaction.
pub fn fire_at(
    w: &mut World,
    shooter: EntityId,
    target: EntityId,
    mount: usize,
    rounds: u8,
) -> Result<u8, String> {
    let source_aa = w.entity(shooter)
        .and_then(|e| e.weapons.get(mount))
        .and_then(|m| w.db.weapons.get(&m.weapon))
        .is_some_and(|d| matches!(d.kind, WeaponKind::Sam | WeaponKind::Gun | WeaponKind::Ciws));
    let result = fire_at_impl(w, shooter, target, mount, rounds);
    if source_aa {
        if let Err(reason) = &result {
            // Missing source adjudication cannot silently buy a NoAA bonus.
            // The helper ignores definite range/arc/stock/reaction refusals.
            super::aircombat2026::note_surface_defense_pending(w, target, reason);
        }
    }
    result
}
fn fire_at_impl(
    w: &mut World,
    shooter: EntityId,
    target: EntityId,
    mount: usize,
    rounds: u8,
) -> Result<u8, String> {
    // Autonomous local control cannot restore a gun/launcher lost through a
    // source land-target casualty. Recheck before any mount or ammo is spent.
    if !super::ground2026::operational(w, shooter) {
        return Err("11.1 source facility weapon function unavailable".into());
    }
    if !super::unmanned2026::can_attack(w, shooter)
        || w.entity(shooter)
            .is_some_and(|e| e.ext.get("nuclear.mission_abort") == Some(&1.))
    {
        return Err("AA source platform cannot attack".into());
    }
    let s = w
        .entity(shooter)
        .filter(|e| e.alive && super::expanded_damage2026::platform_weapons_operational(w, e.id))
        .ok_or("AA weapons unavailable")?;
    let side = s.side;
    let class = s.class.clone();
    let m = s
        .weapons
        .get(mount)
        .filter(|m| m.mounts > 0 && m.ammo > 0)
        .ok_or("AA mount empty")?
        .clone();
    let def = w.db.weapons.get(&m.weapon).ok_or("weapon missing")?.clone();
    if !super::expanded_damage2026::weapon_operational(w, shooter, &def.id)? {
        return Err("AA source physical weapon disabled".into());
    }
    super::nuclear2026::release_gate(w, shooter, &def.id)?;
    if !w.sides[side.index()]
        .contacts
        .values()
        .any(|c| c.about == target && !c.stale && !c.bearing_only && c.quality >= 0.3)
    {
        return Err("8.1 no usable own-side contact".into());
    }
    if super::detection2026::known_category(w, side, target).is_none() {
        return Err("8.1 target category is not established by own sensors".into());
    }
    if let Some(t) = w.entity(target) {
        if !matches!(t.kind, PlatformKind::Aircraft | PlatformKind::Missile)
            || !def.targets.accepts(t.kind)
        {
            return Err("AA target type not admitted".into());
        }
    }
    if def.kind == WeaponKind::Aam {
        return super::aam2026::fire_at(w, shooter, target, mount, rounds);
    }
    if def.kind == WeaponKind::Sam {
        if !crate::sensors::radar_emitting(w.entity(shooter).unwrap())
            || w.entity(shooter).unwrap().systems.radar <= 0.05
        {
            return Err("6.2.1 source radar-guided SAM director unavailable/off".into());
        }
        if !super::operations::sam_reaction_ready(w, shooter, target, &def) {
            return Err("8.1.1 combat-system/HF reaction timer not ready".into());
        }
        let count = rounds.min(2).min(m.ammo.min(2) as u8);
        if count == 0 {
            return Err("no SAM rounds".into());
        }
        let st = w.rules.aaw.as_ref().ok_or("AAW runtime absent")?;
        let mut permit = world_prepare(w, st, shooter, target, &def, count)?;
        // The explicit Ford weapon line lists a single multi-function director; Gen6A SARH has four channels (§6.2.1).
        let profile = weapon_profile(&class, &def.id)?;
        let capacity = profile["channels"]
            .as_u64()
            .ok_or("6.2.1 source director/channel fit missing")? as usize;
        let active_packets: BTreeSet<_> = st
            .shots
            .values()
            .filter(|p| {
                p.permit.shooter == shooter
                    && p.permit.weapon == def.id
                    && p.members
                        .iter()
                        .any(|i| w.entity(*i).is_some_and(|e| e.alive))
            })
            .map(|p| p.identity.clone())
            .collect();
        if active_packets.len() >= capacity {
            return Err("6.2.1 all verified SAM fire-control channels occupied".into());
        }
        let rt_geometry = super::communications2026::received_air_solution(w, shooter, target)?;
        let aim = rt_geometry
            .as_ref()
            .map(|r| r.position)
            .unwrap_or(w.entity(target).unwrap().pos);
        let bearing =
            w.entity(shooter).unwrap().pos.bearing_to(aim) - w.entity(shooter).unwrap().course_deg;
        let arcs = profile["launcher_arcs"]
            .as_array()
            .ok_or("SAM launcher/director source arcs missing")?;
        if !arcs
            .iter()
            .filter_map(Value::as_str)
            .any(|a| arc_allows(a, bearing).unwrap_or(false))
        {
            return Err("2.2.1 no source SAM launcher/director arc bears".into());
        }
        // Prevent an unprinted reload constant: each continuous 30s interval can use one opportunity/channel.
        let interval_key = format!("sam:{shooter:?}:{}:{}", def.id, permit.channel);
        if !can_burst(st, &interval_key, w.tick, 1) {
            return Err("8.1.1 same channel/target still in this engagement increment".into());
        }
        let used = st
            .engagements
            .get(&permit.key)
            .map(|e| e.used[permit.band.family()])
            .unwrap_or(0);
        if matches!(permit.band, Band::Medium1 | Band::Medium2) && used > 0 {
            permit.attack.range_nm = 30.0;
        }
        if matches!(permit.band, Band::Long1 | Band::Long2) && used > 0 {
            permit.attack.range_nm = 60.0;
        }
        let actual_def = verified_weapon_def(&class, &def)?;
        let mut ids = Vec::new();
        for _ in 0..count {
            if let Some(id) = crate::weapons::launch(
                w,
                shooter,
                &actual_def,
                Some(target),
                rt_geometry.as_ref().map(|r| r.position),
            ) {
                ids.push(id);
            }
        }
        if ids.len() != count as usize {
            for id in ids {
                retire(w, id);
            }
            return Err("source SAM packet could not spawn atomically".into());
        }
        let mut state = w.rules.aaw.take().unwrap();
        commit(&mut state, permit.clone(), &ids)?;
        consume_burst(&mut state, &interval_key, w.tick, 1)?;
        w.rules.aaw = Some(state);
        w.entity_mut(shooter).unwrap().weapons[mount].ammo -= count as u16;
        let defense_identity = w.rules.aaw.as_ref().unwrap().shots[&ids[0]].identity.clone();
        super::aircombat2026::note_surface_defense(
            w, target, shooter, &defense_identity,
            super::aircombat2026::SurfaceDefenseKind::Sam { projectiles: ids.clone() },
        );
        super::aviation2026::mark_under_fire(w, target);
        trace(
            w,
            shooter,
            "8.1.1.source_launch",
            json!({"packet":ids,"weapon":def.id,"rounds":count,"budget":permit.budget,"range_band":permit.band,"rt_air_refs":rt_geometry.as_ref().map(|r|&r.refs),"source":"2026 §8.1.1/6.2.1; RT6.3.9 preserves actual receiver reaction/director/channel"}),
        );
        return Ok(count);
    }
    if matches!(def.kind, WeaponKind::Gun | WeaponKind::Ciws) {
        if w.entities.values().any(|e| {
            e.alive
                && e.projectile
                    .as_ref()
                    .is_some_and(|p| p.target == Some(target) && p.launched_by_side == side)
                && w.db
                    .weapons
                    .get(e.projectile.as_ref().unwrap().weapon.as_str())
                    .is_some_and(|d| d.kind == WeaponKind::Sam)
        }) {
            return Err("8.1.10 resolve defensive SAM packet before gunfire".into());
        }
        for i in super::expanded_damage2026::physical_weapon_mounts(w, shooter, mount)? {
            let permit = match world_gun_solution(
                w,
                w.rules.aaw.as_ref().unwrap(),
                shooter,
                target,
                &def,
                i,
            ) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let mut state = w.rules.aaw.take().unwrap();
            let outcome = gun_roll(w, &mut state, &permit)?;
            w.rules.aaw = Some(state);
            w.entity_mut(shooter).unwrap().weapons[mount].ammo -= 1;
            super::expanded_damage2026::record_weapon_expenditure(w, shooter, &def.id, 1);
            let source_aa_rating = n(&tables()["gun_fits"][format!("{class}:{}", def.id)]["aa_strength"]);
            super::aircombat2026::note_surface_defense(
                w, target, shooter, &outcome.identity,
                super::aircombat2026::SurfaceDefenseKind::Gun { source_aa_rating },
            );
            super::aviation2026::mark_under_fire(w, target);
            trace(
                w,
                shooter,
                "8.1.3.source_gun",
                json!({"physical_mount":i,"weapon":def.id,"outcome":outcome}),
            );
            if outcome.hit {
                super::operations::intercept(w, target, shooter, &def);
                let t = w.entity(target).unwrap();
                if t.kind.is_projectile() {
                    // The source gun/CIWS outcome physically destroyed this
                    // carrier. Neutral packet retirement is a different cause.
                    super::nuclear2026::removed(w, target, super::nuclear2026::Removal::CarrierDestroyed);
                    retire(w, target);
                    w.sides[side.index()]
                        .contacts
                        .retain(|_, c| c.about != target);
                } else {
                    let handled =
                        super::expanded_damage2026::aircraft_hit(w, target, &outcome.identity);
                    if !handled || w.entity(target).is_some_and(|e| e.hp <= 0.) {
                        crate::damage::sink(w, target);
                    }
                }
            }
            if active(w) {
                // Manual and automatic fire share this own-side receipt.
                // Emit after adjudication so its original calls and rolls keep
                // their order; the receipt contains no enemy ID or outcome.
                w.emit(
                    crate::EventKind::RulesEvent {
                        domain: "aaw2026".into(),
                        rule: "8.1.3.gun_release".into(),
                        side: Some(side),
                        unit: Some(shooter),
                        contact: None,
                        detail: json!({"physical_mount":i,"weapon":def.id,"rounds":1}).to_string(),
                    },
                    crate::Visibility::Side(side),
                );
            }
            return Ok(1);
        }
        return Err("8.1.3 no legal source AA mount/burst/munition available".into());
    }
    Err("AA weapon mechanism not source-bound".into())
}

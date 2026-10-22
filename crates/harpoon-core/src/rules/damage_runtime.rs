//! HV2026 damage in continuous time. Source tables and declared RTS changes:
//! data/rules/damage-runtime-v2.json. Legacy worlds deserialize to Legacy.
use crate::{
    db::{SensorKind, WeaponDef, WeaponKind},
    types::PlatformKind,
    world::Entity,
    EntityId, EventKind, Rng, Side, Visibility, World,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleProfile {
    #[default]
    #[serde(rename = "rts-legacy-v1")]
    Legacy,
    #[serde(rename = "hv2026-rts-v2")]
    Rts2026,
    #[serde(rename = "hv2026-rts-v3")]
    Rts2026Operations,
}
impl RuleProfile {
    pub fn is_legacy(&self) -> bool {
        *self == Self::Legacy
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Runtime {
    pub profile: RuleProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operations: Option<super::operations::State>,
    pub manual_sequences: BTreeMap<EntityId, u64>,
    pub in_batch: bool,
    pub pending: Vec<Hit>,
    pub resolved_hits: BTreeSet<String>,
    pub casualties: BTreeMap<EntityId, Vec<Casualty>>,
    pub control_clocks: BTreeMap<EntityId, [i64; 2]>,
    pub delayed_orders: Vec<DelayedOrder>,
    pub event_sequences: BTreeMap<String, u64>,
    pub small_craft_window: BTreeMap<EntityId, Vec<(i64, f64)>>,
    pub pending_adjudications: BTreeMap<EntityId, PendingAdjudication>,
}
/// Preserve the complete terminal event when a required rule value is unknown.
/// Removing the flight object from movement is neither a hit nor a miss.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingAdjudication {
    pub tick: i64,
    pub rule: String,
    pub target: EntityId,
    pub projectile: Entity,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DelayedOrder {
    pub due: i64,
    pub side: Side,
    pub command: crate::Command,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Casualty {
    pub identity: String,
    pub flood: bool,
    pub severity: f64,
    pub next_damage: i64,
}
impl Runtime {
    pub fn legacy_untouched(&self) -> bool {
        self.profile == RuleProfile::Legacy
            && self.operations.is_none()
            && self.table_fingerprint.is_none()
            && self.manual_sequences.is_empty()
            && !self.in_batch
            && self.pending.is_empty()
            && self.resolved_hits.is_empty()
            && self.casualties.is_empty()
            && self.control_clocks.is_empty()
            && self.delayed_orders.is_empty()
            && self.event_sequences.is_empty()
            && self.small_craft_window.is_empty()
            && self.pending_adjudications.is_empty()
    }
    pub fn validate_tables(&self) -> Result<(), String> {
        if self.profile == RuleProfile::Legacy {
            return Ok(());
        }
        if self.table_fingerprint.as_deref() == Some(fingerprint_for(self.profile).as_str())
            && (self.profile != RuleProfile::Rts2026Operations || self.operations.is_some()) {
            Ok(())
        } else {
            Err("damage rule table revision differs or is missing; use the matching engine instead of re-adjudicating this saved game".into())
        }
    }
}
pub fn fingerprint_for(profile: RuleProfile) -> String {
    if profile == RuleProfile::Rts2026Operations {
        super::operations::fingerprint(&table_fingerprint())
    } else { table_fingerprint() }
}
pub fn table_fingerprint() -> String {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../data/rules/damage-runtime-v2.json"
    ))
    .expect("damage tables");
    let h = serde_json::to_vec(&value)
        .unwrap()
        .iter()
        .fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ *b as u64).wrapping_mul(0x100000001b3)
        });
    format!("fnv64:{h:016x}")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hit {
    pub target: EntityId,
    pub weapon: WeaponDef,
    pub underwater: bool,
    pub bearing: Option<f64>,
    pub shooter: Option<Side>,
    pub identity: String,
    pub tick: i64,
    #[serde(default)]
    pub secondary: bool,
    #[serde(default)]
    pub torpedo_deep: bool,
    #[serde(default)]
    pub impact_area: Option<u8>,
    #[serde(default)]
    pub targeted_sensor: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlatformDamage {
    #[serde(default)]
    pub size: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub year: u16,
    #[serde(default)]
    pub belt_cm: f64,
    #[serde(default)]
    pub deck_cm: f64,
    #[serde(default)]
    pub tps: f64,
    #[serde(default)]
    pub dp_breaks: Vec<f64>,
    #[serde(default)]
    pub speed_steps: Vec<f64>,
    #[serde(default)]
    pub nuclear: bool,
    #[serde(default)]
    pub double_hull: bool,
    #[serde(default)]
    pub aip: bool,
    #[serde(default)]
    pub single_engine: bool,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub mount_locations: BTreeMap<String, String>,
    #[serde(default)]
    pub critical_armor: BTreeMap<String, f64>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WeaponDamage {
    pub ship_dp: Option<f64>,
    pub sub_dp: Option<f64>,
    pub penetration_cm: Option<f64>,
    #[serde(default)]
    pub fuze: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub extra: String,
    #[serde(default)]
    pub airburst: String,
    #[serde(default)]
    pub first_sensor: bool,
    #[serde(default)]
    pub diameter_mm: f64,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub min_launch_altitude_m: f64,
    #[serde(default)]
    pub shaped_charge: bool,
    #[serde(default)]
    pub bomb_kind: String,
    #[serde(default)]
    pub terminal_nm: f64,
    #[serde(default)]
    pub terminal_speed_kts: f64,
    pub range_nm: Option<f64>,
    pub min_range_nm: Option<f64>,
    pub speed_kts: Option<f64>,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub terminal_profile: String,
    #[serde(default)]
    pub prh_generation: u8,
    pub shutdown_hit_factor: Option<f64>,
}
#[derive(Deserialize)]
struct Row {
    through: u8,
    effect: String,
    armored: bool,
}
#[derive(Deserialize)]
struct TimeRules {
    tactical_s: i64,
    intermediate_s: i64,
    surface_s: i64,
    sinking_die_s: i64,
}
#[derive(Deserialize)]
struct Tables {
    critical_ratio: Vec<Vec<u8>>,
    critical_columns: BTreeMap<String, Vec<Row>>,
    platforms: BTreeMap<String, PlatformDamage>,
    weapons: BTreeMap<String, WeaponDamage>,
    severity_limits: BTreeMap<String, [f64; 3]>,
    dc_reduction: Vec<Vec<i8>>,
    narrow_aspect_deg: f64,
    tps_belt_reduction: Vec<[f64; 2]>,
    tps_areas_per_side: u8,
    pressure_hull_chances: [u8; 4],
    shaped_charge_roll_modifier: u8,
    time: TimeRules,
}
fn data() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../data/rules/damage-runtime-v2.json"
        ))
        .expect("damage tables")
    })
}
pub fn active(w: &World) -> bool {
    w.rules.profile != RuleProfile::Legacy
}
pub fn event_identity(w: &mut World, actor: EntityId, purpose: &str) -> String {
    let key = format!("{}:{purpose}", actor.0);
    let sequence = w.rules.event_sequences.entry(key.clone()).or_default();
    *sequence += 1;
    format!("{}:{key}:{}", w.tick, *sequence)
}
fn tt() -> i64 {
    data().time.tactical_s
}
fn it() -> i64 {
    data().time.intermediate_s
}
pub fn surface_seconds() -> i64 {
    data().time.surface_s
}
pub fn platform(class: &str) -> Option<&'static PlatformDamage> {
    data().platforms.get(class)
}
fn platform_of(e: &Entity) -> Option<&PlatformDamage> {
    e.rule_data.as_ref().or_else(|| platform(&e.class))
}
pub fn weapon(id: &str) -> Option<&'static WeaponDamage> {
    data().weapons.get(id)
}
pub fn prh_generation(def: &WeaponDef) -> u8 {
    weapon_spec(def).prh_generation
}
pub fn prh_shutdown_factor(def: &WeaponDef) -> Option<f64> {
    let spec = weapon_spec(def);
    match spec.prh_generation {
        1 => Some(0.0),
        2 => Some(0.25), // §7.4.2 explicitly specifies generation2.
        _ => spec.shutdown_hit_factor,
    }
}
pub fn defer_prh_shutdown(w: &mut World, projectile: EntityId, target: EntityId) {
    if let Some(p) = w.entity(projectile).cloned() {
        w.rules.pending_adjudications.insert(
            projectile,
            PendingAdjudication {
                tick: w.tick,
                rule: "7.4.2.prh_generation3_shutdown".into(),
                target,
                projectile: p,
            },
        );
        trace(
            w,
            target,
            "adjudication_blocked",
            serde_json::json!({
                "rule":"7.4.2.prh_generation3_shutdown", "projectile":projectile,
                "reason":"No verified shutdown hit modifier; complete terminal event saved without a hit/miss roll"
            }),
        );
    }
}
pub fn flight_seconds(def: &WeaponDef, distance_nm: f64) -> f64 {
    let spec = weapon_spec(def);
    let terminal = if spec.terminal_speed_kts > 0.0 {
        distance_nm.min(spec.terminal_nm)
    } else {
        0.0
    };
    (distance_nm - terminal).max(0.0) / def.speed_kts.max(1.0) * 3600.0
        + terminal / spec.terminal_speed_kts.max(1.0) * 3600.0
}
pub fn active_target_radar(w: &World, id: EntityId) -> Option<String> {
    let e = w.entity(id)?;
    if !crate::sensors::radar_emitting(e) || e.systems.radar <= 0.05 {
        return None;
    }
    e.sensors
        .iter()
        .find(|s| {
            w.db.sensors
                .get(&s.sensor)
                .is_some_and(|d| d.kind == SensorKind::Radar)
                && !crate::sensors::sensor_damaged(e, &s.sensor)
        })
        .map(|s| s.sensor.clone())
}
/// Apply the published rows only to a newly created v2 world's database.
/// Historical World snapshots already contain their own unmodified database.
pub fn configure_database(db: &mut crate::db::Database) {
    for (id, attrs) in &data().platforms {
        if let Some(c) = db.platforms.get_mut(id) {
            c.rule_data = Some(attrs.clone());
        }
    }
    if let Some(base) = db.weapons.get("cn_yj12").cloned() {
        for id in data().weapons.keys().filter(|id| id.starts_with("cn_yj12")) {
            let mut def = base.clone();
            def.id = id.clone();
            db.weapons.insert(id.clone(), def);
        }
        for p in db
            .platforms
            .values_mut()
            .filter(|p| p.kind == PlatformKind::Aircraft)
        {
            for fit in &mut p.weapons {
                if fit.weapon == "cn_yj12" {
                    fit.weapon = "cn_yj12_air".into();
                }
            }
        }
    }
    for (id, attrs) in &data().weapons {
        if let Some(d) = db.weapons.get_mut(id) {
            d.rule_data = Some(attrs.clone());
            if let Some(dp) = attrs.ship_dp {
                d.damage = dp;
            }
            if !attrs.airburst.is_empty() {
                d.damage = 0.0;
            }
            if let Some(v) = attrs.range_nm {
                d.range_nm = v;
            }
            if let Some(v) = attrs.min_range_nm {
                d.min_range_nm = v;
            }
            if let Some(v) = attrs.speed_kts {
                d.speed_kts = v;
            }
            if !attrs.display_name.is_empty() {
                d.name = attrs.display_name.clone();
            }
        }
    }
}
pub fn minimum_release_altitude(def: &WeaponDef) -> f64 {
    let s = weapon_spec(def);
    if s.min_launch_altitude_m > 0.0 {
        s.min_launch_altitude_m
    } else if matches!(def.kind, WeaponKind::Ascm | WeaponKind::Slcm) {
        61.0
    } else {
        0.0
    }
}
pub fn cruise_altitude(def: &WeaponDef) -> f64 {
    match weapon_spec(def).profile.as_str() {
        "vhi" => 16000.0,
        "high" => 10000.0,
        _ => 8.0,
    }
}
pub fn update_missile_profile(e: &mut Entity, def: &WeaponDef, range_to_target: f64) {
    if !marker(e) {
        return;
    }
    let spec = weapon_spec(def);
    if spec.terminal_nm > 0.0 && range_to_target <= spec.terminal_nm {
        e.altitude_m = 8.0;
        if spec.terminal_speed_kts > 0.0 {
            e.speed_kts = spec.terminal_speed_kts;
        }
    }
}
fn weapon_spec(def: &WeaponDef) -> WeaponDamage {
    // §14.1.5 explicitly requires the annex's additional-dice notation.
    // Empty notation must not be reconstructed from WeaponKind or raw DP.
    def.rule_data
        .clone()
        .or_else(|| weapon(&def.id).cloned())
        .unwrap_or_default()
}
pub fn marker(e: &Entity) -> bool {
    e.ext.get("rules.v2") == Some(&1.0)
}
fn val(e: &Entity, k: &str) -> f64 {
    e.ext.get(k).copied().unwrap_or(0.0)
}
/// An RTS navigation order includes its route or station. The whole order is
/// delayed once; continuous steering along an accepted route is not re-queued.
pub fn delay_navigation_order(w: &mut World, side: Side, command: &crate::Command) -> bool {
    if crate::exercise::locked(w, side) {
        return false;
    }
    use crate::Command::*;
    let unit = match command {
        SetCourse { unit, .. }
        | SetSpeed { unit, .. }
        | Goto { unit, .. }
        | Stop { unit }
        | Follow { unit, .. }
        | ClearStation { unit } => *unit,
        _ => return false,
    };
    let Some(e) = w
        .entity(unit)
        .filter(|e| e.side == side && e.alive && marker(e))
    else {
        return false;
    };
    let delay = val(e, "damage.bridge_order_delay") as i64;
    let corrected = val(e, "damage.bridge_until") as i64;
    if delay == 0 && corrected <= w.tick {
        return false;
    }
    let due = w.tick.max(corrected) + delay;
    w.rules.delayed_orders.push(DelayedOrder {
        due,
        side,
        command: command.clone(),
    });
    trace(
        w,
        unit,
        "bridge_order_queued",
        serde_json::json!({"due_tick":due,"command":command}),
    );
    true
}
pub fn due_orders(w: &mut World) -> Vec<DelayedOrder> {
    let now = w.tick;
    let mut ready = Vec::new();
    let mut waiting = Vec::new();
    for order in std::mem::take(&mut w.rules.delayed_orders) {
        if order.due <= now {
            ready.push(order)
        } else {
            waiting.push(order)
        }
    }
    w.rules.delayed_orders = waiting;
    ready
}
pub fn bridge_holds_order(w: &mut World, order: &DelayedOrder) -> bool {
    use crate::Command::*;
    let unit = match &order.command {
        SetCourse { unit, .. }
        | SetSpeed { unit, .. }
        | Goto { unit, .. }
        | Stop { unit }
        | Follow { unit, .. }
        | ClearStation { unit } => *unit,
        _ => return false,
    };
    if let Some(e) = w.entity(unit).filter(|e| bridge_holds(e, w.tick)) {
        let due = val(e, "damage.bridge_until") as i64 + val(e, "damage.bridge_order_delay") as i64;
        let mut again = order.clone();
        again.due = due;
        w.rules.delayed_orders.push(again);
        true
    } else {
        false
    }
}
pub fn bridge_holds(e: &Entity, now: i64) -> bool {
    marker(e) && val(e, "damage.bridge_until") > now as f64
}
pub fn forced_depth(e: &Entity) -> Option<f64> {
    if !marker(e) || e.kind != PlatformKind::Sub {
        return None;
    }
    if massive(e)
        || val(e, "damage.doomed") > 0.0
        || major(e, false)
        || val(e, "damage.flood_must_surface") > 0.0
    {
        return Some(0.0);
    }
    if major(e, true) {
        return Some(50.0);
    }
    let n = val(e, "damage.hull_deformation");
    if n >= 2.0 {
        Some(50.0)
    } else if n >= 1.0 {
        Some(100.0)
    } else {
        None
    }
}
pub fn configure_control(w: &mut World, id: EntityId, action: &str) -> Result<(), String> {
    if !active(w) {
        return Err("damage-control commands require the v2 rule profile".into());
    }
    let Some(t) = w
        .entity(id)
        .filter(|e| e.alive && matches!(e.kind, PlatformKind::Ship | PlatformKind::Sub))
        .cloned()
    else {
        return Err("damage control needs a surviving ship or submarine".into());
    };
    match action {
        "flood_magazines" => {
            let e = w.entity_mut(id).unwrap();
            e.ext.insert("damage.magazines_flooded".into(), 1.0);
            for m in &mut e.weapons {
                m.ammo = 0;
            }
        }
        "purge_aviation" => {
            if !aviation(&t) {
                return Err("aviation fuel/ordnance purge requires an aviation ship".into());
            }
            if let Some(cs) = w.rules.casualties.get_mut(&id) {
                cs.retain(|c| !c.identity.ends_with(":aviation_fuel"));
            }
            let e = w.entity_mut(id).unwrap();
            e.ext.insert("damage.aviation_stores_purged".into(), 1.0);
            e.ext.remove("damage.aviation_fuel");
            sync_casualties(w, id);
        }
        "reinforce_dc" => {
            let now = w.tick;
            let e = w.entity_mut(id).unwrap();
            e.ext
                .entry("damage.dc_crew_due".into())
                .or_insert((now + it()) as f64);
            e.ext.insert("damage.dc_weapons_crew".into(), 1.0);
        }
        _ => return Err(
            "unknown damage-control action; use flood_magazines, purge_aviation or reinforce_dc"
                .into(),
        ),
    }
    trace(
        w,
        id,
        "damage_control_order",
        serde_json::json!({"action":action,"persistent":true}),
    );
    Ok(())
}
fn sync_casualties(w: &mut World, id: EntityId) {
    let (fire, flood) = w
        .rules
        .casualties
        .get(&id)
        .map(|cs| {
            (
                cs.iter()
                    .filter(|c| !c.flood)
                    .map(|c| c.severity)
                    .sum::<f64>(),
                cs.iter()
                    .filter(|c| c.flood)
                    .map(|c| c.severity)
                    .sum::<f64>(),
            )
        })
        .unwrap_or((0.0, 0.0));
    if let Some(e) = w.entity_mut(id) {
        e.ext.insert("damage.fire_pct".into(), fire);
        e.ext.insert("damage.flood_pct".into(), flood);
        e.burn_s = fire;
        e.flooding_s = flood;
    }
    if let Some(clocks) = w.rules.control_clocks.get_mut(&id) {
        if fire <= 0.0 {
            clocks[0] = 0;
        }
        if flood <= 0.0 {
            clocks[1] = 0;
        }
    }
}
fn role(e: &Entity) -> &str {
    if let Some(p) = platform_of(e) {
        if !p.role.is_empty() {
            return &p.role;
        }
    }
    // Explicit abstract training classes; having a helicopter pad is NOT a CV.
    match e.class.as_str() {
        "cvn" | "cn_liaoning" | "cn_shandong" | "us_nimitz" | "us_reagan" | "us_america_lha"
        | "us_wasp_lhd" => "aviation",
        "mgb" | "pb" => "small_combatant",
        _ => "surface",
    }
}
fn size(e: &Entity) -> &str {
    platform_of(e)
        .map(|p| p.size.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(match e.class.as_str() {
            "cvn" => "A",
            "mgb" => "F",
            "pb" => "G",
            _ => "C",
        })
}
pub fn aviation(e: &Entity) -> bool {
    role(e) == "aviation"
}
fn severity_limits(e: &Entity) -> [f64; 3] {
    let mut limits = data()
        .severity_limits
        .get(size(e))
        .copied()
        .unwrap_or([8.0, 12.0, 14.0]);
    let year = platform_of(e).map(|p| p.year).unwrap_or(0);
    let adjustment = if year == 0 {
        0.0
    } else if year < 1908 {
        -2.0
    } else if year < 1925 {
        -1.0
    } else if year < 1942 {
        0.0
    } else if year < 1960 {
        1.0
    } else {
        2.0
    };
    for v in &mut limits {
        *v += adjustment;
    }
    limits
}
fn major(e: &Entity, flood: bool) -> bool {
    val(
        e,
        if flood {
            "damage.flood_pct"
        } else {
            "damage.fire_pct"
        },
    ) > severity_limits(e)[0]
}
pub fn massive(e: &Entity) -> bool {
    marker(e) && matches!(e.kind, PlatformKind::Ship | PlatformKind::Sub) && e.hp <= e.hp_max * 0.25
}

pub fn speed_factor(e: &Entity) -> f64 {
    if !marker(e) || !matches!(e.kind, PlatformKind::Ship | PlatformKind::Sub) {
        return 1.0;
    }
    let lost = (e.hp_max - e.hp).max(0.0);
    let step = if let Some(p) = platform_of(e).filter(|p| p.dp_breaks.len() == 6) {
        p.dp_breaks
            .iter()
            .take(5)
            .enumerate()
            .filter(|(_, v)| lost >= **v)
            .map(|(i, _)| i)
            .last()
            .unwrap_or(0)
    } else if lost >= e.hp_max * 0.9 {
        4
    } else {
        ((lost / e.hp_max.max(1.0) * 4.0).floor() as usize).min(3)
    };
    let step = (step + val(e, "damage.engine_levels") as usize).min(4);
    let mut factor = if let Some(p) = platform_of(e).filter(|p| p.speed_steps.len() == 6) {
        p.speed_steps[step] / p.speed_steps[0].max(1.0)
    } else if step == 4 {
        0.0
    } else {
        1.0 - step as f64 * 0.25
    };
    if val(e, "damage.power_lost") > 0.0 {
        factor = 0.0;
    }
    if val(e, "damage.rudder_disabled") > 0.0 {
        factor = factor.min(1.0 / 3.0);
    }
    if major(e, false) || major(e, true) {
        if let Some(c) = platform_of(e).filter(|p| !p.speed_steps.is_empty()) {
            factor = factor.min(15.0 / c.speed_steps[0].max(1.0));
        }
    }
    factor
}
pub fn weapons_operational(e: &Entity) -> bool {
    e.alive
        && !massive(e)
        && (!marker(e)
            || (val(e, "damage.power_lost") == 0.0
                && val(e, "damage.doomed") == 0.0
                && val(e, "damage.personnel") < 4.0
                && val(e, "damage.dc_weapons_crew") == 0.0))
}
pub fn flight_operational(e: &Entity, launch: bool, now: i64) -> bool {
    if !e.alive || massive(e) {
        return false;
    }
    // A legacy undifferentiated casualty must be acknowledged conservatively.
    if val(e, "flight_deck_hit") > 0.0 && val(e, "damage.deck_location_known") == 0.0 {
        return false;
    }
    if val(
        e,
        if launch {
            "damage.deck_launch"
        } else {
            "damage.deck_recover"
        },
    ) > 0.0
    {
        return false;
    }
    val(e, "damage.airplot_until") <= now as f64
        && !major(e, false)
        && val(e, "damage.power_lost") == 0.0
}
fn trace(w: &mut World, id: EntityId, rule: &str, detail: serde_json::Value) {
    let side = w.entity(id).map(|e| e.side);
    w.emit(
        EventKind::RulesEvent {
            domain: "damage".into(),
            rule: rule.into(),
            side,
            unit: Some(id),
            contact: None,
            detail: detail.to_string(),
        },
        side.map(Visibility::Side).unwrap_or(Visibility::Referee),
    );
}

/// Bound adjudication identity strings. Saturation salvos and multi-generation
/// secondary explosions compound identity chains (`a:initial_secondary:...`);
/// the full chain otherwise lands in resolved_hits keys, batch ids and every
/// trace payload, reaching megabyte strings per roll. Shorten once at each hit
/// entry point: identity stays deterministic (prefix + full-string FNV-1a) so
/// RNG derives and dedup keys remain reproducible.
fn short_tag(s: &str) -> String {
    if s.len() <= 96 {
        return s.to_string();
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    let mut cut = 64;
    while !s.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}#{h:016x}", &s[..cut])
}
fn die(w: &mut World, id: EntityId, r: &mut Rng, sides: u8, identity: &str, label: &str) -> u8 {
    let roll = (r.next_f64() * sides as f64) as u8 + 1;
    trace(
        w,
        id,
        "damage_roll",
        serde_json::json!({"profile":"hv2026-rts-v2","hit":identity,"purpose":label,"die":sides,"roll":roll}),
    );
    roll
}
fn choose_index(
    w: &mut World,
    id: EntityId,
    r: &mut Rng,
    n: usize,
    identity: &str,
    label: &str,
) -> usize {
    let roll = (r.next_f64() * n as f64) as usize;
    trace(
        w,
        id,
        "damage_roll",
        serde_json::json!({"profile":"hv2026-rts-v2","hit":identity,"purpose":label,"die":n,"roll":roll+1}),
    );
    roll
}
fn dice(
    w: &mut World,
    id: EntityId,
    r: &mut Rng,
    expression: &str,
    identity: &str,
    label: &str,
) -> u8 {
    if expression.is_empty() {
        return 0;
    }
    let d = die(w, id, r, 6, identity, label);
    match expression {
        "D6" => d,
        "D6/2" => d / 2,
        "D6/3" => d / 3,
        "D6+2" => d + 2,
        "D6+3" => d + 3,
        "D6-2" => d.saturating_sub(2),
        _ => 0,
    }
}
pub fn begin(w: &mut World) {
    if active(w) {
        assert!(!w.rules.in_batch, "nested damage batch");
        w.rules.in_batch = true;
    }
}
pub fn finish(w: &mut World) {
    if !active(w) {
        return;
    }
    w.rules.in_batch = true;
    while !w.rules.pending.is_empty() {
        let hits = std::mem::take(&mut w.rules.pending);
        resolve(w, hits);
    }
    w.rules.in_batch = false;
}
pub fn queue(
    w: &mut World,
    target: EntityId,
    def: &WeaponDef,
    underwater: bool,
    bearing: Option<f64>,
    shooter: Option<Side>,
    origin: Option<String>,
) {
    queue_with_parameters(
        w, target, def, underwater, bearing, shooter, origin, false, None, None,
    );
}
pub fn queue_with_parameters(
    w: &mut World,
    target: EntityId,
    def: &WeaponDef,
    underwater: bool,
    bearing: Option<f64>,
    shooter: Option<Side>,
    origin: Option<String>,
    torpedo_deep: bool,
    impact_area: Option<u8>,
    targeted_sensor: Option<String>,
) {
    if !w.entity(target).is_some_and(|t| t.alive) {
        return;
    }
    let identity = short_tag(&origin.unwrap_or_else(|| {
        let seq = w.rules.manual_sequences.entry(target).or_default();
        *seq += 1;
        format!("manual:{}:{}:{}", w.tick, target.0, *seq)
    }));
    let key = format!("{}:{identity}", target.0);
    if !w.rules.resolved_hits.insert(key) {
        return;
    }
    w.rules.pending.push(Hit {
        target,
        weapon: def.clone(),
        underwater,
        bearing,
        shooter,
        identity,
        tick: w.tick,
        secondary: false,
        torpedo_deep,
        impact_area,
        targeted_sensor,
    });
    if !w.rules.in_batch {
        finish(w);
    }
}
pub fn configure_torpedo(
    w: &mut World,
    id: EntityId,
    weapon: &str,
    deep: bool,
) -> Result<(), String> {
    if !active(w) {
        return Err("torpedo run settings require v2 rules".into());
    }
    let e = w
        .entity(id)
        .filter(|e| e.alive && e.weapons.iter().any(|m| m.weapon == weapon))
        .ok_or("weapon not fitted on surviving unit")?;
    let _ = e;
    let def = w.db.weapons.get(weapon).ok_or("unknown weapon")?;
    if !matches!(def.kind, WeaponKind::Torpedo | WeaponKind::Lwt)
        || weapon_spec(def).fuze != "contact"
    {
        return Err("run setting requires a verified contact-fuzed torpedo".into());
    }
    w.entity_mut(id).unwrap().ext.insert(
        format!("weapon.{weapon}.deep"),
        if deep { 1.0 } else { 0.0 },
    );
    trace(
        w,
        id,
        "torpedo_run_order",
        serde_json::json!({"weapon":weapon,"deep":deep}),
    );
    Ok(())
}
fn queue_secondary(w: &mut World, t: &Entity, dp: f64, flood: bool, identity: String) {
    if dp <= 0.0 {
        return;
    }
    let identity = short_tag(&identity);
    let Some(mut def) = w.db.weapons.values().next().cloned() else {
        return;
    };
    def.id = if flood {
        "secondary_flood"
    } else {
        "secondary_fire"
    }
    .into();
    def.name = if flood { "Flooding" } else { "Fire" }.into();
    def.kind = WeaponKind::Rocket;
    def.damage = dp;
    def.rule_data = None;
    let key = format!("{}:{identity}", t.id.0);
    if !w.rules.resolved_hits.insert(key) {
        return;
    }
    w.rules.pending.push(Hit {
        target: t.id,
        weapon: def,
        underwater: flood,
        bearing: None,
        shooter: None,
        identity,
        tick: w.tick,
        secondary: true,
        torpedo_deep: false,
        impact_area: None,
        targeted_sensor: None,
    });
    if !w.rules.in_batch {
        finish(w);
    }
}
fn caliber(def: &WeaponDef) -> f64 {
    if !def.kind.is_gun_like() {
        return 1000.0;
    }
    let n = def.name.find("mm").or_else(|| def.id.find("mm"));
    if let Some(n) = n {
        let text = if def.name.contains("mm") {
            &def.name
        } else {
            &def.id
        };
        let digits: String = text[..n]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        return digits.parse().unwrap_or(1000.0);
    }
    1000.0
}

#[derive(Clone)]
struct Resolved {
    hit: Hit,
    dp: f64,
    penetrated: bool,
    airburst: bool,
    auto_flood: bool,
    flood_half: bool,
}
fn primary(w: &mut World, h: Hit, t: &Entity) -> Resolved {
    let spec = weapon_spec(&h.weapon);
    let airburst = !spec.airburst.is_empty();
    let mut dp = if t.kind == PlatformKind::Sub {
        spec.sub_dp.unwrap_or(h.weapon.damage)
    } else {
        spec.ship_dp.unwrap_or(h.weapon.damage)
    };
    let mut pen = true;
    let mut auto_flood = false;
    let mut flood_half = false;
    if h.secondary {
        return Resolved {
            dp: h.weapon.damage,
            hit: h,
            penetrated: true,
            airburst: false,
            auto_flood: false,
            flood_half: false,
        };
    }
    if airburst {
        dp = 0.0;
        pen = platform_of(&t).is_none_or(|p| p.deck_cm < 1.0 && p.belt_cm < 1.0);
    } else if h.underwater {
        // Influence under-keel attacks bypass both belt armor and TPS.
        if spec.fuze == "contact" {
            let a = h.bearing.unwrap_or(90.0).abs();
            let narrow = a <= data().narrow_aspect_deg || a >= 180.0 - data().narrow_aspect_deg;
            if narrow {
                dp *= 0.5;
            } else if h.torpedo_deep
                && t.kind == PlatformKind::Ship
                && !matches!(size(t), "A" | "B")
            {
                dp = 0.0;
                pen = false;
                trace(
                    w,
                    t.id,
                    "deep_torpedo_passes_below_small_ship",
                    serde_json::json!({"hit":h.identity,"size":size(t)}),
                );
            } else if let Some(p) = platform_of(&t) {
                if !h.torpedo_deep {
                    let reduction = data()
                        .tps_belt_reduction
                        .iter()
                        .find(|row| p.belt_cm <= row[0])
                        .map(|row| row[1])
                        .unwrap_or(0.5);
                    dp *= 1.0 - reduction;
                } else if p.tps > 0.0 {
                    let side = if h.bearing.unwrap_or(90.0) < 0.0 {
                        "port"
                    } else {
                        "starboard"
                    };
                    let key = format!("damage.tps.{side}");
                    let remaining = t.ext.get(&key).copied().unwrap_or(p.tps);
                    let mut r = w.rng.derive(Rng::hash_tag(
                        h.tick,
                        t.id.0,
                        &format!("tps_area:{}", h.identity),
                    ));
                    let area = h.impact_area.unwrap_or_else(|| {
                        die(
                            w,
                            t.id,
                            &mut r,
                            data().tps_areas_per_side,
                            &h.identity,
                            "RTS14.tps_area",
                        ) - 1
                    });
                    let area_key = format!("{key}.area.{area}");
                    let used = val(t, &area_key) > 0.0;
                    let absorbed = if used {
                        0.0
                    } else {
                        dp.min(p.tps / 3.0).min(remaining)
                    };
                    flood_half = !used && remaining > 0.0 && dp <= 2.0 * remaining;
                    dp -= absorbed;
                    if let Some(e) = w.entity_mut(t.id) {
                        e.ext.insert(key, remaining - absorbed);
                        e.ext.insert(area_key, 1.0);
                    }
                    pen = dp > 0.0;
                    trace(
                        w,
                        t.id,
                        "tps_absorption",
                        serde_json::json!({"hit":h.identity,"side":side,"area":area,"area_already_used":used,"before":remaining,"absorbed":absorbed,"after":remaining-absorbed,"hull_dp":dp}),
                    );
                }
            }
            auto_flood = pen;
        }
    } else if let Some(p) = platform_of(&t) {
        let skim = spec.profile == "skim"
            || spec.terminal_profile == "skim"
            || (spec.profile.is_empty() && h.weapon.guidance == crate::db::Guidance::SeaSkimmer);
        let armor = if skim { p.belt_cm } else { p.deck_cm };
        pen = armor == 0.0 || spec.penetration_cm.is_some_and(|v| v > armor);
        if !pen {
            dp *= 0.5;
        }
    }
    Resolved {
        hit: h,
        dp,
        penetrated: pen,
        airburst,
        auto_flood,
        flood_half,
    }
}

fn column(t: &Entity, h: &Resolved) -> &'static str {
    if h.airburst {
        return "airburst";
    }
    if t.kind == PlatformKind::Sub && t.depth_m > 0.0 {
        return "sub_major";
    }
    if h.hit.underwater {
        return "underwater";
    }
    match role(t) {
        "aviation" => "aviation",
        "small_combatant" => "small_combatant",
        "small_cargo" => "small_cargo",
        "merchant" => "merchant",
        _ => {
            if matches!(
                h.hit.weapon.kind,
                WeaponKind::Ascm | WeaponKind::Slcm | WeaponKind::Sam | WeaponKind::Aam
            ) {
                "missile"
            } else if platform_of(&t).is_some_and(|p| p.year > 0 && p.year < 1955) {
                "pre1955"
            } else {
                "surface"
            }
        }
    }
}
fn ratio_count(w: &mut World, t: &Entity, dp: f64, identity: &str) -> u8 {
    if dp < t.hp_max * 0.01 {
        return 0;
    }
    let denominator = if matches!(size(t), "F" | "G") {
        t.hp_max
    } else {
        (t.hp - dp).max(0.01)
    };
    let ratio = dp / denominator;
    let mut r = w
        .rng
        .derive(Rng::hash_tag(w.tick, t.id.0, &format!("ratio:{identity}")));
    let d = die(w, t.id, &mut r, 6, identity, "14.1.2.damage_ratio");
    // Express thresholds as DP comparisons rather than subtracting recurring
    // binary fractions (e.g. (1.4-1.0)/0.2 is just below2 in f64).
    let row = (0..=10)
        .rev()
        .find(|n| threshold_met(dp * 10.0, denominator * *n as f64))
        .unwrap();
    let extra = if ratio > 1.0 {
        (0..=100)
            .rev()
            .find(|n| threshold_met(dp * 5.0, denominator * (5 + *n) as f64))
            .unwrap_or(0) as u8
    } else {
        0
    };
    let count = data().critical_ratio[row.min(10)][d as usize - 1].saturating_add(extra);
    trace(
        w,
        t.id,
        "critical_ratio",
        serde_json::json!({"hit":identity,"damage":dp,"remaining":denominator,"ratio":ratio,"criticals":count}),
    );
    if ratio >= 3.0 {
        if let Some(e) = w.entity_mut(t.id) {
            e.hp = e.hp.min(t.hp_max * 0.1);
        }
    }
    count
}
fn threshold_met(value: f64, threshold: f64) -> bool {
    value >= threshold
        || (threshold - value) <= f64::EPSILON * 8.0 * value.abs().max(threshold.abs())
}
fn small_craft_damage(w: &mut World, id: EntityId, damage: f64) -> f64 {
    let now = w.tick;
    let history = w.rules.small_craft_window.entry(id).or_default();
    history.retain(|(at, _)| now - *at < tt());
    if let Some((at, total)) = history.last_mut().filter(|(at, _)| *at == now) {
        let _ = at;
        *total += damage;
    } else {
        history.push((now, damage));
    }
    history.iter().map(|(_, dp)| *dp).sum()
}
fn resolve(w: &mut World, mut hits: Vec<Hit>) {
    if hits.is_empty() {
        return;
    }
    hits.sort_by(|a, b| (a.target, &a.identity).cmp(&(b.target, &b.identity)));
    let mut groups: BTreeMap<EntityId, (Entity, Vec<Resolved>)> = BTreeMap::new();
    for h in hits {
        let Some(t) = w.entity(h.target).cloned() else {
            continue;
        };
        if !t.alive {
            continue;
        }
        let result = primary(w, h, &t);
        groups
            .entry(t.id)
            .or_insert_with(|| (t.clone(), vec![]))
            .1
            .push(result);
    }
    // Primary DP for every victim precedes any functional casualties.
    let now = w.tick;
    for (id, (t, hs)) in &groups {
        let total: f64 = hs.iter().map(|h| h.dp).sum();
        let small_total = if matches!(size(t), "F" | "G") {
            small_craft_damage(w, *id, total)
        } else {
            0.0
        };
        if let Some(e) = w.entity_mut(*id) {
            e.hp = t.hp - total;
            if matches!(size(t), "F" | "G") {
                e.hp = if small_total >= t.hp_max * 2.0 {
                    0.0
                } else {
                    t.hp
                };
            }
            if t.kind == PlatformKind::Aircraft && total > 0.0 {
                e.hp = 0.0;
            }
            e.ext.insert("damage.last_hit".into(), now as f64);
        }
        for h in hs {
            let hp = w.entity(*id).map(|e| e.hp.max(0.0)).unwrap_or(0.0);
            w.emit(
                EventKind::Impact {
                    side: t.side,
                    unit: *id,
                    unit_name: t.name.clone(),
                    weapon: h.hit.weapon.name.clone(),
                    damage: h.dp,
                    hp_left: hp,
                },
                Visibility::Side(t.side),
            );
            crate::damage::bda_to_shooter(w, *id, &h.hit.weapon, h.dp, hp, h.hit.shooter);
            trace(
                w,
                *id,
                "hit_resolved",
                serde_json::json!({"hit":h.hit.identity,"weapon":h.hit.weapon.id,"dp":h.dp,"penetrated":h.penetrated,"airburst":h.airburst,"column":column(t,h)}),
            );
        }
    }
    for (id, (t, hs)) in &groups {
        if matches!(t.kind, PlatformKind::Aircraft | PlatformKind::Base) {
            continue;
        }
        let total: f64 = hs.iter().map(|h| h.dp).sum();
        let batch_id = short_tag(
            &hs.iter()
                .map(|h| h.hit.identity.as_str())
                .collect::<Vec<_>>()
                .join("|"),
        );
        let mut suppress = BTreeSet::new();
        for h in hs {
            let spec = weapon_spec(&h.hit.weapon);
            let mut r = w.rng.derive(Rng::hash_tag(
                h.hit.tick,
                id.0,
                &format!("automatic:{}", h.hit.identity),
            ));
            if !h.hit.secondary
                && t.kind == PlatformKind::Sub
                && t.depth_m > 0.0
                && matches!(h.hit.weapon.kind, WeaponKind::Torpedo | WeaponKind::Lwt)
                && spec.diameter_mm > 0.0
            {
                let large_double = platform_of(t).is_some_and(|p| p.double_hull && p.size == "A");
                let chance = if spec.diameter_mm >= 533.0 {
                    if large_double {
                        data().pressure_hull_chances[1]
                    } else {
                        data().pressure_hull_chances[0]
                    }
                } else if large_double {
                    data().pressure_hull_chances[3]
                } else {
                    data().pressure_hull_chances[2]
                };
                let modifier = if spec.shaped_charge {
                    data().shaped_charge_roll_modifier
                } else {
                    0
                };
                let roll = die(
                    w,
                    *id,
                    &mut r,
                    100,
                    &h.hit.identity,
                    "14.1.2.automatic_pressure_hull",
                );
                let adjusted = roll as u16 + modifier as u16;
                let penetrated = adjusted <= chance as u16;
                trace(
                    w,
                    *id,
                    "pressure_hull_automatic_result",
                    serde_json::json!({"hit":h.hit.identity,"chance_percent":chance.saturating_sub(modifier),"threshold":chance,"roll":roll,"roll_modifier":modifier,"adjusted_roll":adjusted,"penetrated":penetrated}),
                );
                if penetrated {
                    effect(
                        w,
                        t,
                        h,
                        "hull_penetration",
                        &mut r,
                        &format!("{}:automatic_hull", h.hit.identity),
                    );
                    suppress.insert(h.hit.identity.clone());
                }
            }
            let threshold = if matches!(h.hit.weapon.kind, WeaponKind::Torpedo | WeaponKind::Lwt) {
                8
            } else if matches!(
                h.hit.weapon.kind,
                WeaponKind::Bomb | WeaponKind::Ascm | WeaponKind::Slcm
            ) {
                4
            } else {
                0
            };
            if !h.hit.secondary
                && (t.kind == PlatformKind::Ship || t.kind == PlatformKind::Sub && t.depth_m == 0.0)
                && h.dp >= 0.6 * t.hp_max
                && threshold > 0
            {
                let roll = die(w, *id, &mut r, 10, &h.hit.identity, "14.1.2.keel_break");
                let broken = roll <= threshold;
                if broken {
                    let e = w.entity_mut(*id).unwrap();
                    e.hp = 0.0;
                    e.ext.insert("damage.instant_sink".into(), 1.0);
                }
                trace(
                    w,
                    *id,
                    "keel_break_result",
                    serde_json::json!({"hit":h.hit.identity,"roll":roll,"chance_percent":threshold*10,"broken":broken}),
                );
            }
        }
        let n = ratio_count(w, t, total, &batch_id);
        let mut chooser = w.rng.derive(Rng::hash_tag(
            w.tick,
            id.0,
            &format!("critical_source:{batch_id}"),
        ));
        let mut ordinals: BTreeMap<String, u8> = BTreeMap::new();
        for i in 0..n {
            let draw = chooser.next_f64();
            let mut pick = draw * total;
            let h = hs
                .iter()
                .find(|h| {
                    pick -= h.dp;
                    pick < 0.0
                })
                .unwrap_or(&hs[0]);
            trace(
                w,
                *id,
                "critical_source_selected",
                serde_json::json!({"draw":draw,"hit":h.hit.identity,"ordinal":i,"total_dp":total}),
            );
            if suppress.contains(&h.hit.identity) {
                continue;
            }
            let ordinal = ordinals.entry(h.hit.identity.clone()).or_default();
            *ordinal += 1;
            let contact_stern = weapon_spec(&h.hit.weapon).fuze == "contact"
                && h.hit
                    .bearing
                    .is_some_and(|a| a.abs() >= 180.0 - data().narrow_aspect_deg);
            let wake = h.hit.weapon.guidance == crate::db::Guidance::WakeHome;
            let forced = if h.hit.underwater
                && (t.kind == PlatformKind::Ship || t.depth_m == 0.0)
                && *ordinal <= 2
            {
                if wake {
                    Some(if *ordinal == 1 {
                        "rudder"
                    } else {
                        "engineering"
                    })
                } else if contact_stern {
                    Some(if *ordinal == 1 {
                        "engineering"
                    } else {
                        "rudder"
                    })
                } else {
                    None
                }
            } else {
                None
            };
            critical(w, t, h, &format!("{batch_id}:ratio:{i}"), forced);
        }
        for h in hs {
            let spec = weapon_spec(&h.hit.weapon);
            let mut r = w.rng.derive(Rng::hash_tag(
                h.hit.tick,
                id.0,
                &format!("extras:{}", h.hit.identity),
            ));
            if h.auto_flood {
                effect(
                    w,
                    t,
                    h,
                    if h.flood_half {
                        "half_flooding"
                    } else {
                        "flooding"
                    },
                    &mut r,
                    &format!("{}:automatic_flood", h.hit.identity),
                );
            }
            if h.airburst {
                let n = dice(
                    w,
                    *id,
                    &mut r,
                    &spec.airburst,
                    &h.hit.identity,
                    "7.4.2.airburst_count",
                );
                for i in 0..n {
                    critical(
                        w,
                        t,
                        h,
                        &format!("{}:airburst:{i}", h.hit.identity),
                        if i == 0 && spec.first_sensor {
                            Some("radar")
                        } else {
                            None
                        },
                    );
                }
            } else if !spec.extra.is_empty() {
                let n = dice(
                    w,
                    *id,
                    &mut r,
                    &spec.extra,
                    &h.hit.identity,
                    "14.1.5.airframe_extra",
                );
                if h.hit.weapon.damage >= 10.0 {
                    effect(
                        w,
                        t,
                        h,
                        "fire",
                        &mut r,
                        &format!("{}:automatic_fire", h.hit.identity),
                    );
                }
                for i in 0..n {
                    critical(w, t, h, &format!("{}:extra:{i}", h.hit.identity), None);
                }
            }
        }
    }
    for id in groups.keys() {
        if let Some(e) = w.entity_mut(*id) {
            if massive(e) && e.kind == PlatformKind::Sub {
                e.ordered_depth_m = 0.0;
            }
        }
        if w.entity(*id).is_some_and(|e| e.hp <= 0.0) {
            settle_loss(w, *id);
        }
    }
}

/// §8.1.5: half the source airframe critical count; no ordinary DP, no automatic fire.
pub fn intercept_fragments(w:&mut World,target:EntityId,missile:&WeaponDef,identity:String)->Result<(),String> {
    let identity = short_tag(&identity);
    let Some(spec)=missile.rule_data.clone().or_else(||weapon(&missile.id).cloned()) else {
        return Err("8.1.5: missile airframe critical notation not verified".into());
    };
    let t=w.entity(target).filter(|e|e.alive).cloned().ok_or("fragment target absent")?;
    if !w.rules.resolved_hits.insert(format!("{}:{identity}",target.0)){return Ok(());}
    let mut r=w.rng.derive(Rng::hash_tag(w.tick,target.0,&identity));
    let count=dice(w,target,&mut r,&spec.extra,&identity,"8.1.5.airframe_critical_count")/2;
    let h=Resolved{hit:Hit{target,weapon:missile.clone(),underwater:false,bearing:None,shooter:None,identity:identity.clone(),tick:w.tick,
        secondary:false,torpedo_deep:false,impact_area:None,targeted_sensor:None},dp:0.0,
        penetrated:platform_of(&t).is_none_or(|p|p.deck_cm<1.0&&p.belt_cm<1.0),airburst:true,auto_flood:false,flood_half:false};
    trace(w,target,"8.1.5.fragment_count",serde_json::json!({"identity":identity,"count":count,"primary_dp":0,"automatic_fire":false}));
    for i in 0..count {critical(w,&t,&h,&format!("{identity}:fragment:{i}"),None);}
    Ok(())
}

fn critical(w: &mut World, t: &Entity, h: &Resolved, identity: &str, forced: Option<&str>) {
    let mut r = w.rng.derive(Rng::hash_tag(h.hit.tick, t.id.0, identity));
    if h.airburst && !h.penetrated {
        trace(
            w,
            t.id,
            "airburst_fragments_blocked_by_armor",
            serde_json::json!({"hit":identity,"source":"English2026 14-2 note5 /14.1.4"}),
        );
        return;
    }
    if let Some(f) = forced {
        effect(w, t, h, f, &mut r, identity);
        return;
    }
    let c = column(t, h);
    let d = die(w, t.id, &mut r, 100, identity, "critical_type");
    let row = data().critical_columns[c]
        .iter()
        .find(|row| d <= row.through)
        .expect("complete D100 column");
    let mut kind = row.effect.as_str();
    if row.armored && !h.penetrated {
        trace(
            w,
            t.id,
            "critical_blocked_by_armor",
            serde_json::json!({"hit":identity,"column":c,"roll":d,"effect":kind}),
        );
        return;
    }
    let cal = caliber(&h.hit.weapon);
    if cal <= 65.0 && !matches!(size(t), "F" | "G") {
        if !h.penetrated {
            return;
        }
        let allowed = match kind {
            "weapon" | "bridge" => true,
            "sensor" => cal >= 12.7,
            "fire" => cal >= 30.0,
            "cic" | "flooding" => cal >= 57.0,
            _ => false,
        };
        if !allowed {
            return;
        }
    }
    if kind == "weapon_flightdeck" {
        kind = if aviation(t) { "flight_deck" } else { "weapon" };
    }
    if kind == "bridge_airplot" {
        kind = if aviation(t) && die(w, t.id, &mut r, 6, identity, "bridge_or_airplot") >= 4 {
            "airplot"
        } else {
            "bridge"
        };
    }
    if let Some(thickness) = platform_of(t).and_then(|p| p.critical_armor.get(kind)) {
        if h.airburst
            || !weapon_spec(&h.hit.weapon)
                .penetration_cm
                .is_some_and(|pen| pen > *thickness)
        {
            trace(
                w,
                t.id,
                "critical_blocked_by_partial_armor",
                serde_json::json!({"hit":identity,"effect":kind,"armor_cm":thickness}),
            );
            return;
        }
    }
    effect(w, t, h, kind, &mut r, identity);
}

fn fire(
    w: &mut World,
    t: &Entity,
    h: &Resolved,
    r: &mut Rng,
    identity: &str,
    flood: bool,
    minus: u8,
    half: bool,
) {
    let personnel = val(w.entity(t.id).unwrap(), "damage.personnel");
    let raw = die(
        w,
        t.id,
        r,
        6,
        identity,
        if flood {
            "flood_severity"
        } else {
            "fire_severity"
        },
    ) + if !flood && personnel >= 3.0 { 2 } else { 0 };
    let light_flood = if flood && (57.0..=65.0).contains(&caliber(&h.hit.weapon)) {
        2
    } else {
        0
    };
    let mut severity = raw.saturating_sub(minus + light_flood) as f64;
    if !h.penetrated {
        severity *= 0.5;
    }
    if caliber(&h.hit.weapon) <= 65.0 {
        severity *= 0.5;
    }
    if half {
        severity *= 0.5;
    }
    if severity <= 0.0 {
        return;
    }
    let immediate = flood && h.hit.underwater || matches!(size(t), "F" | "G");
    let now = w.tick;
    if let Some(e) = w.entity_mut(t.id) {
        let key = if flood {
            "damage.flood_pct"
        } else {
            "damage.fire_pct"
        };
        *e.ext.entry(key.into()).or_default() += severity;
        let due = now + if immediate { it() } else { 3 * tt() };
        let k = if flood {
            "damage.flood_due"
        } else {
            "damage.fire_due"
        };
        let old = e.ext.get(k).copied().unwrap_or(due as f64);
        e.ext.insert(k.into(), old.min(due as f64));
        if flood {
            e.flooding_s = severity;
        } else {
            e.burn_s = severity;
        }
    }
    w.rules.casualties.entry(t.id).or_default().push(Casualty {
        identity: identity.into(),
        flood,
        severity,
        next_damage: now + if immediate { it() } else { 3 * tt() },
    });
    let clocks = w.rules.control_clocks.entry(t.id).or_default();
    let clock = &mut clocks[if flood { 1 } else { 0 }];
    *clock = if *clock == 0 {
        now + 3 * tt()
    } else {
        (*clock).min(now + 3 * tt())
    };
    if immediate {
        queue_secondary(
            w,
            t,
            t.hp_max * severity / 100.0,
            flood,
            format!("{identity}:initial_secondary"),
        );
    }
}
fn effect(w: &mut World, t: &Entity, h: &Resolved, kind: &str, r: &mut Rng, identity: &str) {
    if !w.entity(t.id).is_some_and(|e| e.alive) {
        return;
    }
    let now = w.tick;
    let mut label = kind.to_string();
    match kind {
        "fire" | "half_fire" => fire(w, t, h, r, identity, false, 0, kind == "half_fire"),
        "flooding" | "half_flooding" => {
            fire(w, t, h, r, identity, true, 0, kind == "half_flooding")
        }
        "engineering" => {
            let n =
                if t.kind == PlatformKind::Sub || platform_of(t).is_some_and(|p| p.single_engine) {
                    2.0
                } else {
                    1.0
                };
            let count = val(w.entity(t.id).unwrap(), "damage.engine_hits") as u8 + 1;
            let power = die(w, t.id, r, 6, identity, "engineering_electrical")
                <= match count {
                    1 => 1,
                    2 => 3,
                    _ => 5,
                };
            let duration = if power {
                die(w, t.id, r, 6, identity, "electrical_duration") as i64 * it()
            } else {
                0
            };
            if let Some(e) = w.entity_mut(t.id) {
                *e.ext.entry("damage.engine_levels".into()).or_default() += n;
                e.ext.insert("damage.engine_hits".into(), count as f64);
                if power {
                    e.ext
                        .insert("damage.power_lost".into(), (now + duration) as f64);
                }
            }
            fire(w, t, h, r, identity, false, 2, false);
        }
        "flight_deck" => {
            if !aviation(t) {
                return;
            }
            let location = die(w, t.id, r, 6, identity, "flight_deck_location");
            if let Some(e) = w.entity_mut(t.id) {
                e.ext.insert("flight_deck_hit".into(), 1.0);
                e.ext.insert("damage.deck_location_known".into(), 1.0);
                e.ext.insert(
                    if location <= 2 {
                        "damage.deck_launch"
                    } else if location >= 5 {
                        "damage.deck_recover"
                    } else {
                        "damage.deck_waist"
                    }
                    .into(),
                    1.0,
                );
            }
            trace(
                w,
                t.id,
                "flight_deck_location",
                serde_json::json!({"hit":identity,"location_die":location,"launch_blocked":location<=2,"recovery_blocked":location>=5}),
            );
            let deck_armor = platform_of(t).map(|p| p.deck_cm).unwrap_or(0.0);
            let can_reach_hangar = h.penetrated
                && (deck_armor == 0.0
                    || h.hit.weapon.kind == WeaponKind::Bomb
                        && weapon_spec(&h.hit.weapon).bomb_kind == "AP");
            if can_reach_hangar
                && die(w, t.id, r, 10, identity, "flight_deck_hangar_penetration") >= 6
            {
                effect(
                    w,
                    t,
                    h,
                    "hangar",
                    r,
                    &format!("{identity}:hangar_penetration"),
                );
            }
        }
        "bridge" | "airplot" => {
            let turns = if matches!(size(t), "F" | "G") {
                1
            } else {
                die(w, t.id, r, 6, identity, "bridge_recovery") as i64 / 2
            };
            if let Some(e) = w.entity_mut(t.id) {
                e.ext.insert(
                    if kind == "airplot" {
                        "damage.airplot_until"
                    } else {
                        "damage.bridge_until"
                    }
                    .into(),
                    (now + turns * tt()) as f64,
                );
                if kind == "bridge" {
                    e.ext.insert(
                        "damage.bridge_order_delay".into(),
                        if matches!(size(t), "F" | "G") {
                            0.0
                        } else {
                            (2 * tt()) as f64
                        },
                    );
                    e.ext.insert("damage.bridge_course".into(), e.course_deg);
                    e.ext.insert("damage.bridge_speed".into(), e.speed_kts);
                    e.waypoints.clear();
                    e.station = None;
                    e.ordered_course_deg = e.course_deg;
                    e.ordered_speed_kts = e.speed_kts;
                    for s in &t.sensors {
                        if s.sensor.contains("nav") {
                            e.ext.insert(format!("damage.sensor.{}", s.sensor), 1.0);
                        }
                    }
                    if e.kind == PlatformKind::Sub {
                        e.ordered_depth_m = 25.0;
                        e.assigned = None;
                    }
                }
            }
            if kind == "bridge" && t.kind == PlatformKind::Sub {
                if let Some(e) = w.entity_mut(t.id) {
                    for key in e
                        .ext
                        .keys()
                        .filter(|k| k.starts_with("firectl."))
                        .cloned()
                        .collect::<Vec<_>>()
                    {
                        e.ext.remove(&key);
                    }
                }
                if die(w, t.id, r, 10, identity, "bridge_submarine_broach") == 1 {
                    if let Some(e) = w.entity_mut(t.id) {
                        e.depth_m = 0.0;
                        e.ext
                            .insert("damage.broach_until".into(), (now + tt()) as f64);
                    }
                }
            }
            fire(w, t, h, r, identity, false, 2, false);
        }
        "rudder" => {
            let disabled = die(w, t.id, r, 6, identity, "rudder_mode") >= 4;
            let repair = if disabled {
                0
            } else {
                die(w, t.id, r, 6, identity, "rudder_first_unjam_delay") as i64 * tt()
            };
            let turn =
                w.db.platforms
                    .get(&t.class)
                    .map(|c| c.turn_rate_deg_s)
                    .unwrap_or(0.0);
            if let Some(e) = w.entity_mut(t.id) {
                e.ext.insert("rudder_hit".into(), 1.0);
                e.ext.insert(
                    if disabled {
                        "damage.rudder_disabled"
                    } else {
                        "damage.rudder_jammed"
                    }
                    .into(),
                    1.0,
                );
                if !disabled {
                    e.ext.insert(
                        "damage.rudder_rate".into(),
                        crate::geom::angle_diff(e.course_deg, e.ordered_course_deg)
                            .clamp(-turn, turn),
                    );
                    e.ext
                        .insert("damage.rudder_repair_due".into(), (now + repair) as f64);
                    e.ext.insert("damage.rudder_attempts".into(), 0.0);
                }
            }
        }
        "cic" | "cic_cargo" => {
            let nav_ids: Vec<_> = t
                .sensors
                .iter()
                .filter(|s| s.sensor.contains("nav"))
                .map(|s| s.sensor.clone())
                .collect();
            if let Some(e) = w.entity_mut(t.id) {
                e.systems.fire_control = 0.0;
                e.ext.insert("damage.cic".into(), 1.0);
                for s in &t.sensors {
                    if !nav_ids.contains(&s.sensor) {
                        e.ext.insert(format!("damage.sensor.{}", s.sensor), 1.0);
                    }
                }
            }
        }
        "sensor" | "radar" => {
            let choices: Vec<_> = t
                .sensors
                .iter()
                .filter(|s| {
                    let def = w.db.sensors.get(&s.sensor);
                    (kind != "radar" || def.is_some_and(|d| d.kind == SensorKind::Radar))
                        && (kind != "radar"
                            || h.hit
                                .targeted_sensor
                                .as_ref()
                                .is_none_or(|id| *id == s.sensor))
                        && (caliber(&h.hit.weapon) > 65.0
                            || def.is_none_or(|d| {
                                !matches!(
                                    d.kind,
                                    SensorKind::SonarActive | SensorKind::SonarPassive
                                )
                            }))
                })
                .collect();
            if choices.is_empty() {
                return;
            }
            let idx = (die(
                w,
                t.id,
                r,
                choices.len() as u8,
                identity,
                "sensor_selection",
            ) - 1) as usize;
            let selected = &choices[idx].sensor;
            let radar =
                w.db.sensors
                    .get(selected)
                    .is_some_and(|s| s.kind == SensorKind::Radar);
            if let Some(e) = w.entity_mut(t.id) {
                e.ext.insert(format!("damage.sensor.{selected}"), 1.0);
                if kind == "radar" {
                    e.ext.insert("damage.arm_radar_hit".into(), 1.0);
                }
            }
            label = if radar { "radar" } else { "sensor" }.into();
        }
        "weapon" | "weapon_cargo" => {
            if t.weapons.is_empty() {
                return;
            }
            let mut choices = Vec::new();
            for (idx, m) in t.weapons.iter().enumerate() {
                let count = val(
                    w.entity(t.id).unwrap(),
                    &format!("damage.mount_count.{idx}"),
                );
                let count = if count > 0.0 {
                    count as usize
                } else {
                    m.mounts as usize
                };
                for physical in 0..count {
                    choices.push((idx, physical));
                }
                w.entity_mut(t.id)
                    .unwrap()
                    .ext
                    .insert(format!("damage.mount_count.{idx}"), count as f64);
            }
            if choices.is_empty() {
                return;
            }
            let (idx, physical) = choices[choose_index(
                w,
                t.id,
                r,
                choices.len(),
                identity,
                "weapon_mount_selection",
            )];
            let mount = &t.weapons[idx];
            let key = format!("damage.mount.{idx}.{physical}");
            if val(w.entity(t.id).unwrap(), &key) > 0.0 {
                return;
            }
            let live = w.entity(t.id).unwrap().weapons[idx].clone();
            if live.mounts == 0 {
                return;
            }
            let per_mount = (live.ammo as f64 / live.mounts as f64).ceil() as u16;
            let explosive = w.db.weapons.get(&mount.weapon).is_some_and(|d| {
                !matches!(d.kind, WeaponKind::Gun | WeaponKind::Ciws) || caliber(d) >= 20.0
            }) && per_mount > 0;
            let explosion = explosive
                && !h.airburst
                && val(w.entity(t.id).unwrap(), "damage.magazines_flooded") == 0.0
                && die(w, t.id, r, 10, identity, "mount_ammunition") == 10;
            let location = platform_of(t)
                .and_then(|p| p.mount_locations.get(&mount.weapon))
                .map(String::as_str);
            let burst_dp = if explosion && location == Some("topside") {
                w.db.weapons
                    .get(&mount.weapon)
                    .map(|d| d.damage * (per_mount as f64 / 2.0).floor().max(1.0))
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            if let Some(e) = w.entity_mut(t.id) {
                e.ext.insert(key, 1.0);
                e.weapons[idx].mounts = live.mounts - 1;
                e.weapons[idx].ammo = live.ammo.saturating_sub(per_mount);
                if explosion && (location == Some("belowdecks") || t.kind == PlatformKind::Sub) {
                    e.hp = 0.0;
                    e.ext.insert("damage.instant_sink".into(), 1.0);
                }
            }
            if explosion && location.is_none() && t.kind != PlatformKind::Sub {
                trace(
                    w,
                    t.id,
                    "critical_requires_mount_location",
                    serde_json::json!({"hit":identity,"weapon":mount.weapon,"missing":"topside/belowdecks magazine location","automatic_explosion_dp_blocked":true}),
                );
            }
            if burst_dp > 0.0 {
                queue_secondary(w, t, burst_dp, false, format!("{identity}:mount_explosion"));
            }
            label = if explosion {
                "mount_explosion"
            } else {
                "weapon_mount"
            }
            .into();
        }
        "hangar" => {
            let n = die(w, t.id, r, 6, identity, "hangar_aircraft");
            let mut destroyed = 0;
            for _ in 0..n {
                let Some(deck) = w.entity(t.id).and_then(|e| e.deck.as_ref()) else {
                    break;
                };
                let count = deck.ready.iter().map(|(_, n)| *n as usize).sum::<usize>()
                    + deck
                        .cycling
                        .iter()
                        .map(|(_, n, _)| *n as usize)
                        .sum::<usize>();
                if count == 0 {
                    break;
                }
                let mut index =
                    choose_index(w, t.id, r, count, identity, "hangar_aircraft_selection");
                let deck = w.entity_mut(t.id).unwrap().deck.as_mut().unwrap();
                let mut found = false;
                for (_, n) in &mut deck.ready {
                    if index < *n as usize {
                        *n -= 1;
                        found = true;
                        break;
                    }
                    index -= *n as usize;
                }
                if !found {
                    for (_, n, _) in &mut deck.cycling {
                        if index < *n as usize {
                            *n -= 1;
                            break;
                        }
                        index -= *n as usize;
                    }
                }
                destroyed += 1;
            }
            for aircraft in 0..destroyed {
                fire(
                    w,
                    t,
                    h,
                    r,
                    &format!("{identity}:aircraft:{aircraft}"),
                    false,
                    2,
                    false,
                );
            }
            trace(
                w,
                t.id,
                "hangar_aircraft_destroyed",
                serde_json::json!({"hit":identity,"rolled":n,"destroyed":destroyed,"airborne_excluded":true}),
            );
        }
        "ammo_fuel" => {
            let ammo = die(w, t.id, r, 10, identity, "aviation_ammo_or_fuel") <= 3;
            if ammo {
                if val(w.entity(t.id).unwrap(), "damage.magazines_flooded") == 0.0
                    && val(w.entity(t.id).unwrap(), "damage.aviation_stores_purged") == 0.0
                    && die(w, t.id, r, 10, identity, "aviation_magazine_explosion") == 10
                {
                    if let Some(e) = w.entity_mut(t.id) {
                        e.hp = 0.0;
                        e.ext.insert("damage.instant_sink".into(), 1.0);
                    }
                }
            } else if val(w.entity(t.id).unwrap(), "damage.aviation_stores_purged") == 0.0 {
                fire(
                    w,
                    t,
                    h,
                    r,
                    &format!("{identity}:aviation_fuel"),
                    false,
                    0,
                    false,
                );
                if let Some(c) = w.rules.casualties.get_mut(&t.id).and_then(|c| c.last_mut()) {
                    c.severity += 2.0;
                }
                w.entity_mut(t.id)
                    .unwrap()
                    .ext
                    .insert("damage.aviation_fuel".into(), 1.0);
                sync_casualties(w, t.id);
            }
        }
        "personnel" | "flotation" => {
            let key = format!("damage.{kind}");
            let n = val(w.entity(t.id).unwrap(), &key) + 1.0;
            if let Some(e) = w.entity_mut(t.id) {
                e.ext.insert(key, n);
                if kind == "flotation" {
                    *e.ext.entry("damage.engine_levels".into()).or_default() += 1.0;
                    if n >= if size(t) == "F" { 5.0 } else { 4.0 } {
                        e.hp = 0.0;
                    }
                } else if n >= 4.0 {
                    e.fire_on = false;
                }
            }
        }
        "hull_penetration" => {
            let surface = t.depth_m <= 100.0
                && die(w, t.id, r, 10, identity, "pressure_hull_surface_escape") <= 5;
            let delay = if surface {
                die(w, t.id, r, 10, identity, "pressure_hull_sink_delay") as i64 * 2 * tt()
            } else {
                0
            };
            if let Some(e) = w.entity_mut(t.id) {
                if surface {
                    e.ext.insert("damage.doomed".into(), 1.0);
                    e.ext.insert("damage.sink_at".into(), (now + delay) as f64);
                    e.ordered_depth_m = 0.0;
                    e.fire_on = false;
                } else {
                    e.hp = 0.0;
                }
            }
        }
        "hull_deformation" => {
            let n = val(w.entity(t.id).unwrap(), "damage.hull_deformation") + 1.0;
            if let Some(e) = w.entity_mut(t.id) {
                e.ext.insert("damage.hull_deformation".into(), n);
                e.ordered_depth_m = e.ordered_depth_m.min(if n == 1.0 { 100.0 } else { 50.0 });
            }
            if n >= 3.0 {
                effect(w, t, h, "hull_penetration", r, identity);
            }
        }
        "battery" | "battery_engineering" => {
            if kind == "battery_engineering" && platform_of(t).is_some_and(|p| p.nuclear) {
                effect(w, t, h, "engineering", r, identity);
                return;
            }
            if platform_of(t).is_some_and(|p| p.nuclear) {
                return;
            }
            let aip = platform_of(t).is_some_and(|p| p.aip)
                && die(w, t.id, r, 100, identity, "aip_hit") <= 25;
            if aip {
                w.entity_mut(t.id)
                    .unwrap()
                    .ext
                    .insert("damage.aip".into(), 1.0);
            } else {
                let loss = die(w, t.id, r, 6, identity, "battery_capacity_loss") as f64 * 0.05;
                if let Some(e) = w.entity_mut(t.id) {
                    let capacity = e
                        .ext
                        .get("damage.battery_capacity")
                        .copied()
                        .unwrap_or(100.0);
                    let charge = e.ext.get("battery").copied().unwrap_or(capacity);
                    let remaining = capacity * (1.0 - loss);
                    e.ext.insert("damage.battery_capacity".into(), remaining);
                    e.ext
                        .insert("damage.battery_loss".into(), 1.0 - remaining / 100.0);
                    e.ext.insert("battery".into(), charge * (1.0 - loss));
                }
            }
        }
        "control_major" | "control_minor" => {
            if kind == "control_major" {
                effect(w, t, h, "bridge", r, identity);
                effect(w, t, h, "cic", r, identity);
            } else {
                let selected = if die(w, t.id, r, 6, identity, "minor_control_type") <= 3 {
                    "bridge"
                } else {
                    "cic"
                };
                effect(w, t, h, selected, r, identity);
            }
        }
        _ => {
            trace(
                w,
                t.id,
                "critical_not_yet_implemented",
                serde_json::json!({"hit":identity,"effect":kind}),
            );
            return;
        }
    }
    w.emit(
        EventKind::CriticalHit {
            side: t.side,
            unit: t.id,
            system: label,
        },
        Visibility::Side(t.side),
    );
}

#[cfg(test)]
mod source_effect_tests {
    use super::*;
    #[test]
    fn shaped_charge_applies_the_printed_modifier_to_the_roll_not_the_threshold() {
        let mut exercised = false;
        for seed in 0..64 {
            let (mut w, t, h) = sub(seed, 200.0, false);
            let mut def = h.hit.weapon.clone();
            def.kind = WeaponKind::Torpedo;
            def.rule_data = Some(WeaponDamage {
                ship_dp: Some(1.0),
                sub_dp: Some(1.0),
                diameter_mm: 533.0,
                shaped_charge: true,
                ..Default::default()
            });
            queue(&mut w, t.id, &def, true, Some(90.0), None, None);
            let result = w
                .events
                .iter()
                .find_map(|e| match &e.kind {
                    EventKind::RulesEvent { rule, detail, .. }
                        if rule == "pressure_hull_automatic_result" =>
                    {
                        Some(serde_json::from_str::<serde_json::Value>(detail).unwrap())
                    }
                    _ => None,
                })
                .unwrap();
            let roll = result["roll"].as_u64().unwrap();
            if (61..=80).contains(&roll) {
                exercised = true;
            }
            assert_eq!(
                result["penetrated"],
                roll + 20 <= 80,
                "English2026 PDF129 explicitly modifies the die roll, not the80% threshold"
            );
        }
        assert!(exercised);
    }
    #[test]
    fn severe_submarine_flooding_gets_one_control_attempt_before_forced_surface() {
        let mut forced = 0;
        for seed in 0..32 {
            let (mut w, t, _) = sub(seed, 50.0, false);
            let severity = severity_limits(&t)[1] + 0.5;
            w.rules.casualties.insert(
                t.id,
                vec![Casualty {
                    identity: "severe-flood".into(),
                    flood: true,
                    severity,
                    next_damage: 10000,
                }],
            );
            w.rules.control_clocks.insert(t.id, [0, 540]);
            sync_casualties(&mut w, t.id);
            step(&mut w);
            assert_eq!(
                forced_depth(w.entity(t.id).unwrap()),
                Some(50.0),
                "the initial severe casualty grants one control attempt"
            );
            w.tick = 540;
            step(&mut w);
            let e = w.entity(t.id).unwrap();
            if val(e, "damage.flood_pct") > severity_limits(e)[1] {
                forced += 1;
                assert_eq!(
                    forced_depth(e),
                    Some(0.0),
                    "14.4: failed to reduce severe flooding, must surface"
                );
                let restored: World =
                    serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
                assert_eq!(forced_depth(restored.entity(t.id).unwrap()), Some(0.0));
            }
        }
        assert!(forced > 0, "test exercised failed damage control");
    }
    #[test]
    fn ratio_thresholds_preserve_printed_decimal_boundaries() {
        for (damage, remaining, extra) in [(6.0, 5.0, 1), (7.0, 5.0, 2), (8.0, 5.0, 3)] {
            let (mut w, mut t, _) = sub(3, 0.0, false);
            t.hp = damage + remaining;
            t.hp_max = t.hp;
            let mut r = w
                .rng
                .derive(Rng::hash_tag(w.tick, t.id.0, "ratio:boundary"));
            let d = (r.next_f64() * 6.0) as usize;
            assert_eq!(
                ratio_count(&mut w, &t, damage, "boundary"),
                data().critical_ratio[10][d] + extra,
                "14.1.2: each complete0.2 above1.0 adds one critical"
            );
            let below = damage - 0.000001;
            t.hp = below + remaining;
            assert_eq!(
                ratio_count(&mut w, &t, below, "boundary"),
                data().critical_ratio[10][d] + extra - 1,
                "values genuinely below the boundary must not round up"
            );
        }
    }
    #[test]
    fn small_craft_uses_a_tactical_damage_window_not_a_one_second_threshold() {
        let (mut w, t, _) = sub(3, 0.0, false);
        assert_eq!(small_craft_damage(&mut w, t.id, 9.0), 9.0);
        w.tick = 179;
        assert_eq!(small_craft_damage(&mut w, t.id, 11.0), 20.0);
        let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        restored.tick = 180;
        assert_eq!(
            small_craft_damage(&mut restored, t.id, 1.0),
            12.0,
            "the oldest damage expires after one180-second tactical duration"
        );
        restored.tick = 360;
        assert_eq!(small_craft_damage(&mut restored, t.id, 2.0), 2.0);
    }
    fn sub(seed: u64, depth: f64, nuclear: bool) -> (World, Entity, Resolved) {
        let db = crate::Database::built_in();
        let class = &db
            .platforms
            .values()
            .find(|c| c.kind == PlatformKind::Sub)
            .unwrap()
            .id;
        let sc:crate::Scenario=serde_json::from_value(serde_json::json!({"id":"critical-source","name":"critical-source","seed":seed,"duration_s":100000,"weather":{"visibility_nm":12,"jam":[0,0],"overcast":0,"rain":0},"units":[{"side":"Blue","class":class,"pos":{"x":0,"y":0},"depth_m":depth}]})).unwrap();
        let mut w = crate::load_scenario(sc, db).unwrap();
        let id = EntityId(1);
        let e = w.entity_mut(id).unwrap();
        e.rule_data = Some(PlatformDamage {
            nuclear,
            size: "C".into(),
            ..Default::default()
        });
        let t = e.clone();
        let d = w.db.weapons.values().next().unwrap().clone();
        let h = Resolved {
            hit: Hit {
                target: id,
                weapon: d,
                underwater: true,
                bearing: Some(90.0),
                shooter: None,
                identity: "source-critical".into(),
                tick: 0,
                secondary: false,
                torpedo_deep: false,
                impact_area: None,
                targeted_sensor: None,
            },
            dp: 10.0,
            penetrated: true,
            airburst: false,
            auto_flood: false,
            flood_half: false,
        };
        (w, t, h)
    }
    #[test]
    fn shallow_hull_penetration_has_a_rescue_window_but_is_never_repaired() {
        let mut escaped = 0;
        for seed in 0..128 {
            let (mut w, t, h) = sub(seed, 50.0, false);
            let mut r = Rng::new(seed);
            effect(&mut w, &t, &h, "hull_penetration", &mut r, "penetration");
            let e = w.entity(t.id).unwrap();
            if e.hp > 0.0 {
                escaped += 1;
                assert_eq!(val(e, "damage.doomed"), 1.0);
                let sink_at = val(e, "damage.sink_at") as i64;
                assert!((360..=3600).contains(&sink_at) && sink_at % 360 == 0);
                assert!(!weapons_operational(e));
                w.tick = sink_at;
                step(&mut w);
                assert!(!w.entity(t.id).unwrap().alive);
            }
        }
        assert!(
            escaped > 32 && escaped < 96,
            "14.5 shallow/IntI: 50% surface escape, not guaranteed instant sinking"
        );
        let (mut w, t, h) = sub(0, 200.0, false);
        effect(&mut w, &t, &h, "hull_penetration", &mut Rng::new(0), "deep");
        assert!(w.entity(t.id).unwrap().hp <= 0.0);
    }
    #[test]
    fn mixed_battery_row_becomes_engineering_on_nuclear_submarines() {
        let (mut w, t, h) = sub(17, 50.0, true);
        effect(
            &mut w,
            &t,
            &h,
            "battery_engineering",
            &mut Rng::new(17),
            "nuclear",
        );
        assert_eq!(
            val(w.entity(t.id).unwrap(), "damage.engine_levels"),
            2.0,
            "14.5 single-engine compartment loss is two speed steps"
        );
        assert_eq!(val(w.entity(t.id).unwrap(), "damage.battery_loss"), 0.0);
    }
    #[test]
    fn battery_critical_removes_charged_and_depleted_capacity_proportionally() {
        let (mut w, t, h) = sub(9, 50.0, false);
        w.entity_mut(t.id)
            .unwrap()
            .ext
            .insert("battery".into(), 40.0);
        effect(&mut w, &t, &h, "battery", &mut Rng::new(9), "cells");
        let e = w.entity(t.id).unwrap();
        let loss = val(e, "damage.battery_loss");
        assert!((0.05..=0.30).contains(&loss));
        assert!((val(e, "battery") - 40.0 * (1.0 - loss)).abs() < 1e-9);
        assert!((val(e, "damage.battery_capacity") - 100.0 * (1.0 - loss)).abs() < 1e-9);
        assert!(w.events.iter().any(|ev|matches!(&ev.kind,EventKind::RulesEvent{rule,detail,..}if rule=="damage_roll"&&detail.contains("battery_capacity_loss"))));
    }
    #[test]
    fn major_control_damage_applies_both_bridge_and_cic() {
        let (mut w, t, h) = sub(1, 50.0, false);
        effect(&mut w, &t, &h, "control_major", &mut Rng::new(1), "control");
        let e = w.entity(t.id).unwrap();
        assert_eq!(val(e, "damage.cic"), 1.0);
        assert_eq!(val(e, "damage.bridge_order_delay"), 360.0);
    }
    #[test]
    fn shallow_contact_torpedo_uses_belt_and_never_consumes_tps() {
        let (mut w, mut t, mut h) = sub(1, 0.0, false);
        t.kind = PlatformKind::Ship;
        t.rule_data.as_mut().unwrap().tps = 315.0;
        t.rule_data.as_mut().unwrap().size = "A".into();
        t.rule_data.as_mut().unwrap().belt_cm = 15.0;
        h.hit.weapon.rule_data = Some(WeaponDamage {
            fuze: "contact".into(),
            ..Default::default()
        });
        h.hit.weapon.damage = 100.0;
        let result = primary(&mut w, h.hit.clone(), &t);
        assert_eq!(
            result.dp, 75.0,
            "14.1.7 shallow contact +11-20cm belt reduces DP25%; TPS is for deep contact"
        );
        assert!(!w
            .entity(t.id)
            .unwrap()
            .ext
            .keys()
            .any(|s| s.starts_with("damage.tps.")));
    }
    #[test]
    fn deep_contact_tps_is_separate_by_side_and_cannot_absorb_same_area_twice() {
        let (mut w, mut t, mut h) = sub(1, 0.0, false);
        t.kind = PlatformKind::Ship;
        t.rule_data.as_mut().unwrap().tps = 315.0;
        t.rule_data.as_mut().unwrap().size = "A".into();
        *w.entity_mut(t.id).unwrap() = t.clone();
        h.hit.weapon.rule_data = Some(WeaponDamage {
            fuze: "contact".into(),
            ..Default::default()
        });
        h.hit.weapon.damage = 97.0;
        h.hit.torpedo_deep = true;
        h.hit.impact_area = Some(0);
        let first = primary(&mut w, h.hit.clone(), &t);
        assert_eq!(first.dp, 0.0);
        let after = w.entity(t.id).unwrap().clone();
        let second = primary(&mut w, h.hit.clone(), &after);
        assert_eq!(
            second.dp, 97.0,
            "a spent void in the same area cannot protect the vitals a second time"
        );
        h.hit.bearing = Some(-90.0);
        let after = w.entity(t.id).unwrap().clone();
        let port = primary(&mut w, h.hit, &after);
        assert_eq!(port.dp, 0.0, "port protection remains intact");
    }
    #[test]
    fn one_weapon_critical_does_not_destroy_every_mount_in_a_fit() {
        let (mut w, mut t, h) = sub(3, 50.0, false);
        t.weapons.truncate(1);
        t.weapons[0].mounts = 3;
        t.weapons[0].ammo = 0;
        *w.entity_mut(t.id).unwrap() = t.clone();
        effect(&mut w, &t, &h, "weapon", &mut Rng::new(3), "mount-group");
        assert_eq!(
            w.entity(t.id).unwrap().weapons[0].mounts,
            2,
            "14.5 Weapon: one physical mount/cell group, not all installations of this weapon"
        );
    }
    #[test]
    fn conflagration_risk_accumulates_and_flooding_magazines_prevents_detonation() {
        for flooded in [false, true] {
            let (mut w, t, _) = sub(3, 50.0, false);
            w.rules.casualties.insert(
                t.id,
                vec![Casualty {
                    identity: "test-fire".into(),
                    flood: false,
                    severity: 25.0,
                    next_damage: 1000000,
                }],
            );
            sync_casualties(&mut w, t.id);
            if flooded {
                configure_control(&mut w, t.id, "flood_magazines").unwrap();
            }
            w.tick = 1;
            step(&mut w);
            for n in 1..=4 {
                w.tick = 1 + n * 1800;
                step(&mut w);
            }
            assert_eq!(w.entity(t.id).unwrap().alive,flooded,"14.4: 25% cumulative per IT reaches certainty at fourth IT, unless magazines were flooded");
        }
    }
    #[test]
    fn aviation_store_purge_leaves_unrelated_fires_burning() {
        let (mut w, mut t, _) = sub(3, 0.0, false);
        t.rule_data.as_mut().unwrap().role = "aviation".into();
        *w.entity_mut(t.id).unwrap() = t.clone();
        w.rules.casualties.insert(
            t.id,
            vec![
                Casualty {
                    identity: "bridge-fire".into(),
                    flood: false,
                    severity: 2.0,
                    next_damage: 540,
                },
                Casualty {
                    identity: "hit:aviation_fuel".into(),
                    flood: false,
                    severity: 6.0,
                    next_damage: 540,
                },
            ],
        );
        sync_casualties(&mut w, t.id);
        configure_control(&mut w, t.id, "purge_aviation").unwrap();
        assert_eq!(val(w.entity(t.id).unwrap(), "damage.fire_pct"), 2.0);
        assert_eq!(
            val(w.entity(t.id).unwrap(), "damage.aviation_stores_purged"),
            1.0
        );
    }
}

/// Per-casualty clocks use simulation seconds, never wall-clock or UI frames.
pub fn step(w: &mut World) {
    if !active(w) {
        return;
    }
    let ids: Vec<_> = w
        .entities
        .values()
        .filter(|e| e.alive && !e.kind.is_projectile())
        .map(|e| e.id)
        .collect();
    for id in ids {
        let Some(t) = w.entity(id).cloned() else {
            continue;
        };
        let now = w.tick;
        if val(&t, "damage.doomed") > 0.0 && val(&t, "damage.sink_at") <= now as f64 {
            settle_loss(w, id);
            continue;
        }
        if val(&t, "damage.power_lost") > 0.0 && val(&t, "damage.power_lost") <= now as f64 {
            w.entity_mut(id).unwrap().ext.remove("damage.power_lost");
        }
        let attempts = val(&t, "damage.rudder_attempts") as u8;
        let due = val(&t, "damage.rudder_repair_due") as i64;
        if val(&t, "damage.rudder_jammed") > 0.0 && attempts < 3 && due > 0 && due <= now {
            let identity = format!("rudder_unjam:{}:{}", id.0, attempts + 1);
            let mut r = w.rng.derive(Rng::hash_tag(now, id.0, &identity));
            let roll = die(w, id, &mut r, 10, &identity, "14.6.rudder_unjam");
            let success = roll <= 2 * (attempts + 1);
            if let Some(e) = w.entity_mut(id) {
                e.ext
                    .insert("damage.rudder_attempts".into(), (attempts + 1) as f64);
                e.ext
                    .insert("damage.rudder_repair_due".into(), (now + tt()) as f64);
                if success {
                    e.ext.remove("damage.rudder_jammed");
                    e.ext.remove("rudder_hit");
                }
            }
            trace(
                w,
                id,
                "rudder_unjam_result",
                serde_json::json!({"attempt":attempts+1,"roll":roll,"chance_percent":20*(attempts+1),"success":success}),
            );
        }
        let fire = val(&t, "damage.fire_pct");
        let flood = val(&t, "damage.flood_pct");
        let limits = severity_limits(&t);
        let extra_crew =
            val(&t, "damage.dc_crew_due") > 0.0 && val(&t, "damage.dc_crew_due") <= now as f64;
        let effective = (fire + flood - if extra_crew { limits[0] / 2.0 } else { 0.0 }).max(0.0);
        let tier = limits.iter().filter(|limit| effective > **limit).count();
        if let Some(e) = w.entity_mut(id) {
            e.ext.insert(
                "damage.major_fire".into(),
                if fire > limits[0] { 1.0 } else { 0.0 },
            );
            e.ext.insert(
                "damage.major_flood".into(),
                if flood > limits[0] { 1.0 } else { 0.0 },
            );
            e.ext.insert(
                "damage.major_casualty".into(),
                if fire.max(flood) > limits[0] {
                    1.0
                } else {
                    0.0
                },
            );
        }
        for (index, pct) in [(0, "damage.fire_pct"), (1, "damage.flood_pct")] {
            let severity = w
                .rules
                .casualties
                .get(&id)
                .map(|cs| {
                    cs.iter()
                        .filter(|c| c.flood == (index == 1))
                        .map(|c| c.severity)
                        .sum::<f64>()
                })
                .unwrap_or(0.0);
            let due = w
                .rules
                .control_clocks
                .get(&id)
                .map(|c| c[index])
                .unwrap_or(i64::MAX);
            if severity <= 0.0 || due == 0 || due > now {
                continue;
            }
            let identity = format!("dc:{id:?}:{now}:{pct}");
            let mut r = w.rng.derive(Rng::hash_tag(now, id.0, &identity));
            let raw = die(w, id, &mut r, 10, &identity, "14.4.damage_control");
            let modifier = if val(&t, "damage.power_lost") > 0.0 {
                2
            } else {
                0
            } + if val(&t, "damage.aviation_fuel") > 0.0 {
                2
            } else {
                0
            } + if index == 0 && fire > limits[0] && t.speed_kts > 15.0 {
                2
            } else {
                0
            } + if index == 0 && val(&t, "damage.personnel") >= 4.0 {
                2
            } else {
                0
            };
            let roll = (raw + modifier).min(10);
            let count = data().dc_reduction[roll as usize - 1][tier.min(3)];
            let change = (0..count.unsigned_abs())
                .map(|_| die(w, id, &mut r, 6, &identity, "dc_severity_change") as f64)
                .sum::<f64>()
                * count.signum() as f64;
            let remaining = (severity + change).max(0.0);
            if index == 1 && t.kind == PlatformKind::Sub {
                let failed = severity > limits[1] && remaining > limits[1];
                let e = w.entity_mut(id).unwrap();
                if failed {
                    e.ext.insert("damage.flood_must_surface".into(), 1.0);
                } else if remaining <= limits[1] {
                    e.ext.remove("damage.flood_must_surface");
                }
                if severity > limits[1] {
                    trace(
                        w,
                        id,
                        "severe_flood_surface_check",
                        serde_json::json!({
                            "before_pct":severity,"after_pct":remaining,"severe_above":limits[1],"must_surface":failed
                        }),
                    );
                }
            }
            // RTS allocation: preserve each casualty's own damage clock;
            // distribute the aggregate control result proportionally.
            if let Some(cs) = w.rules.casualties.get_mut(&id) {
                for c in cs.iter_mut().filter(|c| c.flood == (index == 1)) {
                    c.severity *= remaining / severity;
                }
            }
            w.rules.control_clocks.entry(id).or_default()[index] =
                if remaining > 0.0 { now + it() } else { 0 };
            trace(
                w,
                id,
                "damage_control_result",
                serde_json::json!({"hit":identity,"raw_roll":raw,"modifier":modifier,"tier":tier,"before_pct":severity,"after_pct":remaining}),
            );
        }
        let mut secondary = Vec::new();
        if let Some(cs) = w.rules.casualties.get_mut(&id) {
            for c in cs.iter_mut() {
                if c.next_damage <= now {
                    secondary.push((
                        t.hp_max * c.severity / 100.0,
                        c.flood,
                        format!("{}:secondary:{now}", c.identity),
                    ));
                    c.next_damage += it();
                }
            }
            cs.retain(|c| c.severity > 0.0);
        }
        let (fire, flood) = w
            .rules
            .casualties
            .get(&id)
            .map(|cs| {
                (
                    cs.iter()
                        .filter(|c| !c.flood)
                        .map(|c| c.severity)
                        .sum::<f64>(),
                    cs.iter()
                        .filter(|c| c.flood)
                        .map(|c| c.severity)
                        .sum::<f64>(),
                )
            })
            .unwrap_or((0.0, 0.0));
        if let Some(e) = w.entity_mut(id) {
            e.ext.insert("damage.fire_pct".into(), fire);
            e.ext.insert("damage.flood_pct".into(), flood);
            e.burn_s = fire;
            e.flooding_s = flood;
        }
        for (dp, flood, identity) in secondary {
            queue_secondary(w, &t, dp, flood, identity);
        }
        for (flood, severity, name) in [(false, fire, "conflagration"), (true, flood, "capsize")] {
            let due_key = format!("damage.{name}_due");
            let count_key = format!("damage.{name}_count");
            let prevented = !flood && val(w.entity(id).unwrap(), "damage.magazines_flooded") > 0.0;
            if severity <= limits[2] || prevented {
                let e = w.entity_mut(id).unwrap();
                e.ext.remove(&due_key);
                e.ext.remove(&count_key);
                continue;
            }
            let due = w
                .entity(id)
                .unwrap()
                .ext
                .get(&due_key)
                .copied()
                .unwrap_or((now + it()) as f64) as i64;
            w.entity_mut(id)
                .unwrap()
                .ext
                .insert(due_key.clone(), due as f64);
            if due > now {
                continue;
            }
            let count = (val(w.entity(id).unwrap(), &count_key) as u8).saturating_add(1);
            let chance = if flood { 25 } else { 25 * count.min(4) };
            let identity = format!("{name}:{}:{now}", id.0);
            let mut r = w.rng.derive(Rng::hash_tag(now, id.0, &identity));
            let roll = die(w, id, &mut r, 100, &identity, "14.4.overwhelmed_casualty");
            let destroyed = roll <= chance;
            let e = w.entity_mut(id).unwrap();
            e.ext.insert(due_key, (now + it()) as f64);
            e.ext.insert(count_key, count as f64);
            if destroyed {
                e.hp = 0.0;
                e.ext.insert("damage.instant_sink".into(), 1.0);
            }
            trace(
                w,
                id,
                "overwhelmed_casualty_result",
                serde_json::json!({"type":name,"interval":count,"chance_percent":chance,"roll":roll,"destroyed":destroyed}),
            );
        }
        if w.entity(id).is_some_and(|e| e.hp <= 0.0) {
            settle_loss(w, id);
        }
    }
}

fn settle_loss(w: &mut World, id: EntityId) {
    let Some(t) = w.entity(id).filter(|e| e.alive).cloned() else {
        return;
    };
    let surface =
        matches!(t.kind, PlatformKind::Ship) || t.kind == PlatformKind::Sub && t.depth_m == 0.0;
    if !surface || val(&t, "damage.instant_sink") > 0.0 {
        crate::damage::sink(w, id);
        return;
    }
    let due = val(&t, "damage.sink_at") as i64;
    if due > 0 {
        if due <= w.tick {
            crate::damage::sink(w, id);
        }
        return;
    }
    let identity = event_identity(w, id, "14.3.sinking");
    let mut r = w.rng.derive(Rng::hash_tag(w.tick, id.0, &identity));
    let roll = die(w, id, &mut r, 10, &identity, "14.3.sinking_minutes");
    let due = w.tick + roll as i64 * data().time.sinking_die_s;
    let e = w.entity_mut(id).unwrap();
    e.ext.insert("damage.doomed".into(), 1.0);
    e.ext.insert("damage.sink_at".into(), due as f64);
    e.ext.insert("damage.sinking".into(), 1.0);
    e.speed_kts = 0.0;
    e.ordered_speed_kts = 0.0;
    e.fire_on = false;
    e.waypoints.clear();
    e.station = None;
    trace(
        w,
        id,
        "sinking_countdown",
        serde_json::json!({"roll":roll,"sink_at":due,"rule":"14.3","combat_ineffective":true}),
    );
}

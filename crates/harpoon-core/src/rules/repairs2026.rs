//! §14.6 source-time repair checks on recorded physical criticals. Successful
//! repair restores a system, never spent ammunition or structural hit points.
use crate::{EntityId, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::OnceLock};

pub fn tables() -> &'static Value {
    static TABLES: OnceLock<Value> = OnceLock::new();
    TABLES.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../data/rules/repairs-2026.json")).unwrap()
    })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub units: BTreeMap<EntityId, Unit>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Unit {
    pub criticals: BTreeMap<String, Critical>,
    pub session: Option<Session>,
    pub pending: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Critical {
    pub identity: String,
    pub occurred_at: i64,
    pub levels: u8,
    pub remaining_levels: u8,
    pub checks: BTreeMap<u8, Check>,
    pub sea_unrepairable: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub began_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previous_checks: Vec<BTreeMap<u8, Check>>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub own_identity: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub began_at: i64,
    pub automatic: bool,
    /// Referee-only cursor, never projected in own_view.
    #[serde(default)]
    pub last_event_seq: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Check {
    pub checkpoint_hours: u8,
    pub due_at: i64,
    pub captured_at: i64,
    pub remaining_dp: f64,
    pub original_dp: f64,
    pub chance: f64,
    pub d100: Option<u8>,
    pub success: Option<bool>,
    pub applied: bool,
}
pub fn record_engineering(w: &mut World, unit: EntityId, identity: &str, levels: u8) {
    record_system(w, unit, identity, "engineering", None, levels)
}
pub fn record_system(
    w: &mut World,
    unit: EntityId,
    identity: &str,
    kind: &str,
    component: Option<&str>,
    levels: u8,
) {
    if w.rules.profile != super::damage_runtime::RuleProfile::Rts2026Full {
        return;
    }
    // Bounded secondary/cascade identities may no longer contain the full
    // original hit key. Preserve each recorded private cause's own opaque ID
    // so projecting a repair ledger cannot merge distinct casualties.
    if w.rules.private_hit_aliases.contains_key(&unit) && identity.contains("mine:") {
        super::damage_runtime::register_private_hit(w, unit, identity);
    }
    super::damage_runtime::register_source_cause(w, unit, identity);
    let Some(state) = w.rules.repairs.as_mut() else {
        return;
    };
    let u = state.units.entry(unit).or_default();
    let key = if kind == "engineering" {
        identity.to_string()
    } else {
        format!("{identity}|{kind}|{}", component.unwrap_or(""))
    };
    if u.criticals.contains_key(&key) {
        return;
    }
    if u.session.is_some() {
        // Re-hit clocks restart only for the actual affected system. Preserve
        // every earlier roll as history; no new check runs before the player
        // declares a new battle end. Unrelated component clocks are retained.
        for c in u.criticals.values_mut().filter(|c| {
            c.remaining_levels > 0 && !c.sea_unrepairable
                && effective_kind(c) == kind
                && c.component.as_deref() == component
        }) {
            if !c.checks.is_empty() {
                c.previous_checks.push(std::mem::take(&mut c.checks));
            }
            c.began_at = None;
            c.sea_unrepairable = false;
        }
        u.pending=Some("14.6 renewed own damage; explicitly declare the new battle end before repairs continue".into());
    }
    let own_identity = format!("own-system-casualty-{}-{}", unit.0, u.criticals.len() + 1);
    u.criticals.insert(
        key,
        Critical {
            identity: identity.into(),
            occurred_at: w.tick,
            levels,
            remaining_levels: levels,
            checks: BTreeMap::new(),
            sea_unrepairable: false,
            kind: if kind == "engineering" {
                String::new()
            } else {
                kind.into()
            },
            component: component.map(str::to_string),
            began_at: None,
            previous_checks: vec![],
            own_identity,
        },
    );
}
fn effective_kind(c: &Critical) -> &str {
    if c.kind.is_empty() {
        "engineering"
    } else {
        &c.kind
    }
}
fn trace(w: &mut World, unit: EntityId, rule: &str, detail: Value) {
    let Some(side) = w.entity(unit).map(|e| e.side) else {
        return;
    };
    w.emit(
        crate::EventKind::RulesEvent {
            domain: "repairs2026".into(),
            rule: rule.into(),
            side: Some(side),
            unit: Some(unit),
            contact: None,
            detail: super::damage_runtime::public_cause_detail(w, unit, &detail).to_string(),
        },
        crate::Visibility::Side(side),
    );
}
fn active(w: &World) -> bool {
    w.rules.profile == super::damage_runtime::RuleProfile::Rts2026Full && w.rules.repairs.is_some()
}
fn has_active_casualty(w: &World, id: EntityId) -> bool {
    w.rules
        .casualties
        .get(&id)
        .is_some_and(|cs| cs.iter().any(|c| c.severity > 0.))
}
fn ledger_matches(w: &World, id: EntityId, u: &Unit) -> bool {
    let Some(e) = w.entity(id) else { return false };
    let actual = e.ext.get("damage.engine_levels").copied().unwrap_or(0.);
    let recorded: u32 = u
        .criticals
        .values()
        .filter(|c| effective_kind(c) == "engineering")
        .map(|c| c.remaining_levels as u32)
        .sum();
    actual.is_finite()
        && (actual - recorded as f64).abs() < 1e-9
        && u.criticals
            .values()
            .all(|c| c.levels > 0 && c.remaining_levels <= c.levels)
}
fn pend(w: &mut World, id: EntityId, u: &mut Unit, reason: &str) {
    if u.pending.is_none() {
        u.pending = Some(reason.into());
        trace(
            w,
            id,
            "14.6.repair_pending",
            json!({"reason":reason,"status":"pending"}),
        );
    }
}
fn continued_combat(w: &World, id: EntityId, cursor: u64) -> bool {
    w.events
        .iter()
        .filter(|e| e.seq > cursor)
        .any(|e| match &e.kind {
            crate::EventKind::WeaponsRelease { shooter, .. }
            | crate::EventKind::GunAction { shooter, .. }
            | crate::EventKind::SamShot { shooter, .. } => *shooter == id,
            crate::EventKind::Impact { unit, .. } => *unit == id,
            _ => false,
        })
}
/// The optional breakdown/aircraft consumers share the actual non-battle
/// declaration. A manual button cannot bypass a pending main repair session.
pub(super) fn optional_session(w: &World, id: EntityId) -> Result<i64, String> {
    let u=w.rules.repairs.as_ref().and_then(|s|s.units.get(&id))
        .ok_or("explicit begin_repairs declaration required")?;
    let session=u.session.as_ref().ok_or("explicit begin_repairs declaration required")?;
    if let Some(reason)=&u.pending {return Err(reason.clone());}
    if continued_combat(w,id,session.last_event_seq)||has_active_casualty(w,id) {
        return Err("14.7 renewed own combat or active fire/flooding prevents non-battle repair".into());
    }
    Ok(session.began_at)
}
pub(super) fn optional_rehit(w: &mut World, id: EntityId) {
    if !active(w) {return;}
    if let Some(u)=w.rules.repairs.as_mut().and_then(|s|s.units.get_mut(&id)) {
        if u.session.is_some() {
            u.pending=Some("14.6 renewed own aircraft damage; explicitly declare the new battle end before repairs continue".into());
        }
    }
}
fn advance(w: &mut World, id: EntityId, u: &mut Unit, manual_roll: bool) -> Result<(), String> {
    let Some(session) = u.session.as_ref() else {
        return Ok(());
    };
    if u.pending.is_some() {
        return Ok(());
    }
    if continued_combat(w, id, session.last_event_seq) || has_active_casualty(w, id) {
        pend(w,id,u,"14.6 renewed own combat/damage control after declared battle end; new repair-clock priority requires adjudication");
        return Ok(());
    }
    if !ledger_matches(w, id, u) {
        pend(w,id,u,"14.6 current Engineering aggregate differs from recorded critical contributions; no legacy reconstruction");
        return Ok(());
    }
    let session = u.session.as_mut().unwrap();
    session.last_event_seq = w.event_seq;
    let began = session.began_at;
    let roll_now = manual_roll || session.automatic;
    let Some(e) = w.entity(id).filter(|e| e.alive) else {
        return Ok(());
    };
    let (remaining, original) = (e.hp, e.hp_max);
    if !remaining.is_finite()
        || !original.is_finite()
        || original <= 0.
        || remaining < 0.
        || remaining > original
    {
        pend(
            w,
            id,
            u,
            "14.6 original/remaining DP is not a valid source ratio",
        );
        return Ok(());
    }
    let hours = tables()["checkpoint_hours"].as_array().unwrap();
    let factors = tables()["chance_factors"].as_array().unwrap();
    for critical in u.criticals.values_mut() {
        if critical.remaining_levels == 0 || critical.sea_unrepairable {
            continue;
        }
        for (index, hour) in hours.iter().enumerate() {
            let hour = hour.as_u64().unwrap() as u8;
            let due = critical.began_at.unwrap_or(began) + hour as i64 * 3600;
            if w.tick < due {
                break;
            }
            // Capture source-time input before manual execution can be delayed.
            let check = critical.checks.entry(hour).or_insert_with(|| Check {
                checkpoint_hours: hour,
                due_at: due,
                captured_at: w.tick,
                remaining_dp: remaining,
                original_dp: original,
                chance: remaining / original * factors[index].as_f64().unwrap(),
                d100: None,
                success: None,
                applied: false,
            });
            if !roll_now || check.d100.is_some() {
                continue;
            }
            let purpose = format!("14.6.repair:{}:{hour}", critical.identity);
            let mut r = w.rng.derive(crate::Rng::hash_tag(0, id.0, &purpose));
            let d100 = 1 + (r.next_f64() * tables()["dice_sides"].as_f64().unwrap()) as u8;
            let success = d100 as f64 / 100. <= check.chance;
            check.d100 = Some(d100);
            check.success = Some(success);
            if success {
                // The root explicitly authorizes this single existing field.
                // Verify against the running contribution total before callers
                // enter this loop; each applied check removes exactly one.
                let kind = if critical.kind.is_empty() {
                    "engineering"
                } else {
                    critical.kind.as_str()
                };
                if kind == "engineering" {
                    let e = w.entity_mut(id).unwrap();
                    let current = e.ext.get("damage.engine_levels").copied().unwrap_or(0.);
                    if current < 1. || !current.is_finite() {
                        return Err("14.6 recorded engineering reduction unavailable; retain original success pending".into());
                    }
                    e.ext.insert("damage.engine_levels".into(), current - 1.);
                } else {
                    super::expanded_damage2026::restore_system(
                        w,
                        id,
                        kind,
                        critical.component.as_deref(),
                    )?;
                }
                critical.remaining_levels -=
                    tables()["engineering_levels_per_success"].as_u64().unwrap() as u8;
                check.applied = true;
            } else if hour == tables()["sea_limit_hours"].as_u64().unwrap() as u8 {
                critical.sea_unrepairable = true;
            }
            trace(
                w,
                id,
                "14.6.repair_check",
                json!({"critical_identity":critical.identity,"checkpoint_hours":hour,"due_at":check.due_at,"captured_at":check.captured_at,"remaining_dp":check.remaining_dp,"original_dp":check.original_dp,"chance":check.chance,"d100":d100,"success":success,"applied":check.applied,"sea_unrepairable":critical.sea_unrepairable,"source":"English2026 PDF136/14-10 section14.6"}),
            );
            if success || critical.sea_unrepairable {
                break;
            }
        }
    }
    Ok(())
}
pub fn control(w: &mut World, unit: EntityId, action: &str) -> Option<Result<(), String>> {
    if !matches!(
        action,
        "begin_repairs" | "repairs_auto" | "repairs_manual" | "repair_due"
    ) {
        return None;
    }
    if !active(w) {
        return Some(Err("14.6 source repairs require explicit v4".into()));
    }
    if w.entity(unit).is_none_or(|e| {
        !e.alive
            || !matches!(
                e.kind,
                crate::types::PlatformKind::Ship
                    | crate::types::PlatformKind::Sub
                    | crate::types::PlatformKind::Aircraft
            )
    }) {
        return Some(Err(
            "14.6 source repairs need an owned surviving ship/submarine".into(),
        ));
    }
    let mut state = w.rules.repairs.take().unwrap();
    let u = state.units.entry(unit).or_default();
    let result = (|| {
        if action == "begin_repairs"
            && u.pending
                .as_deref()
                .is_some_and(|p| p.starts_with("14.6 renewed own"))
            && !has_active_casualty(w, unit)
        {
            u.pending = None;
            u.session = None;
        }
        if let Some(reason) = &u.pending {
            return Err(reason.clone());
        }
        if !ledger_matches(w, unit, u) {
            pend(w,unit,u,"14.6 Engineering damage has no complete per-critical ledger; preserve unknown old damage");
            return Err(u.pending.clone().unwrap());
        }
        if action == "begin_repairs" {
            if u.session.is_some() {
                return Ok(());
            }
            if u.criticals.is_empty()
                && !super::expanded_damage2026::has_recorded_repair_work(w, unit)
            {
                return Err("14.6 no supported recorded system critical".into());
            }
            if has_active_casualty(w, unit) {
                return Err("14.6 ongoing fire/flooding control: post-battle repair coordination requires adjudication".into());
            }
            u.session = Some(Session {
                began_at: w.tick,
                automatic: false,
                last_event_seq: w.event_seq,
            });
            for c in u.criticals.values_mut() {
                if c.began_at.is_none() {
                    c.began_at = Some(w.tick);
                }
            }
            trace(
                w,
                unit,
                "14.6.repairs_begin",
                json!({"began_at":w.tick,"automatic":false,"declaration":"explicit_player_post_battle"}),
            );
            return Ok(());
        }
        let Some(session) = u.session.as_mut() else {
            return Err(
                "14.6 begin_repairs must explicitly declare the post-battle start first".into(),
            );
        };
        if action == "repairs_auto" {
            session.automatic = true;
        }
        if action == "repairs_manual" {
            session.automatic = false;
        }
        advance(w, unit, u, action == "repair_due")
    })();
    w.rules.repairs = Some(state);
    if result.is_ok() {
        trace(w, unit, "14.6.repair_order", json!({"action":action}));
    }
    Some(result)
}
pub fn step(w: &mut World) {
    if !active(w) {
        return;
    }
    let mut state = w.rules.repairs.take().unwrap();
    for (id, u) in &mut state.units {
        if let Err(reason) = advance(w, *id, u, false) {
            pend(w, *id, u, &reason);
        }
    }
    w.rules.repairs = Some(state);
}
pub fn own_view(w: &World, unit: EntityId) -> Option<Value> {
    if w.rules.profile != super::damage_runtime::RuleProfile::Rts2026Full {
        return None;
    }
    let u = w.rules.repairs.as_ref()?.units.get(&unit)?;
    // The event cursor can count hidden foreign events, so it is private even
    // from the damaged unit's own observer.
    let criticals: BTreeMap<_, _> = u
        .criticals
        .iter()
        .map(|(key, c)| {
            let label = if c.own_identity.is_empty() {
                key.as_str()
            } else {
                c.own_identity.as_str()
            };
            let mut value = serde_json::to_value(c).unwrap();
            value["identity"] = json!(if c.own_identity.is_empty() {
                c.identity.as_str()
            } else {
                label
            });
            (label.to_string(), value)
        })
        .collect();
    Some(super::damage_runtime::public_cause_detail(
        w,
        unit,
        &json!({"criticals":criticals,"session":u.session.as_ref().map(|s|json!({"began_at":s.began_at,"automatic":s.automatic})),"pending":u.pending,"scope":"recorded physical-system criticals; no cargo, ammunition or structural DP restoration"}),
    ))
}

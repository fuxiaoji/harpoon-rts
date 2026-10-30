//! §14.1.2 additional gunfire carrier-deck casualty. Referee-only source proof.
use crate::rules::{damage_runtime as dr, surface2026::gunnery::GunImpact};
use crate::{EntityId, Rng, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};
pub fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/automatic-gun-deck-2026.json"
        ))
        .unwrap()
    })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Window {
    pub turn: i64,
    pub target: EntityId,
    pub hit: dr::Hit,
    pub dp: f64,
    pub bands: BTreeSet<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decision {
    pub window: Window,
    pub identity: String,
    pub d100: u8,
    pub chance_percent: Option<u8>,
    pub applied: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub windows: BTreeMap<String, Window>,
    pub last_closed: BTreeMap<EntityId, i64>,
    pub pending: BTreeMap<String, Decision>,
}
impl State {
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty() && self.last_closed.is_empty() && self.pending.is_empty()
    }
}
fn state(w: &mut World) -> Option<&mut State> {
    Some(&mut w.rules.aaw.as_mut()?.surface.gunnery.gun_deck)
}
/// The caller has verified actual penetration/carrier role/deck armor using
/// the common resolver. No player command or free-form packet exposes this.
pub(crate) fn record_actual_gunfire(w: &mut World, hit: &dr::Hit, dp: f64, proof: GunImpact) {
    if proof.range_band < 2
        || proof.range_band > 3
        || proof.caliber_mm < tables()["minimum_caliber_mm"].as_f64().unwrap()
    {
        return;
    }
    let Some(s) = state(w) else { return };
    if s.last_closed
        .get(&hit.target)
        .is_some_and(|turn| *turn >= proof.tactical_turn)
    {
        return;
    }
    let key = format!("{}:{}", hit.target.0, proof.tactical_turn);
    let window = s.windows.entry(key).or_insert_with(|| Window {
        turn: proof.tactical_turn,
        target: hit.target,
        hit: hit.clone(),
        dp,
        bands: BTreeSet::new(),
    });
    window.bands.insert(proof.range_band);
    if hit.identity < window.hit.identity {
        window.hit = hit.clone();
        window.dp = dp;
    }
}
/// Taking a closed window before applying casualties is the terminal-once
/// boundary, preserved across save/restore and multiple common batches.
pub(crate) fn resolve_turn(w: &mut World) -> Vec<Decision> {
    let now = w.tick.div_euclid(
        tables()["execution"]["tactical_turn_seconds"]
            .as_i64()
            .unwrap(),
    );
    let Some(s) = state(w) else { return vec![] };
    let keys: Vec<_> = s
        .windows
        .iter()
        .filter(|(_, v)| v.turn < now)
        .map(|(k, _)| k.clone())
        .collect();
    let windows: Vec<_> = keys
        .into_iter()
        .map(|k| s.windows.remove(&k).unwrap())
        .collect();
    let mut result = vec![];
    for window in windows {
        let identity = format!("14.1.2:gun_deck:{}:{}", window.target.0, window.turn);
        let mut r = Rng::new(w.seed).derive(Rng::hash_tag(window.turn, window.target.0, &identity));
        let d100 = 1 + (r.next_f64() * 100.) as u8;
        let chance_percent = if window.bands.len() == 1 {
            Some(
                tables()[if window.bands.contains(&2) {
                    "long_percent"
                } else {
                    "extreme_percent"
                }]
                .as_u64()
                .unwrap() as u8,
            )
        } else {
            None
        };
        let applied = chance_percent.is_some_and(|p| d100 <= p)
            && w.entity(window.target)
                .is_some_and(|e| e.alive && e.hp > 0.);
        let d = Decision {
            window,
            identity,
            d100,
            chance_percent,
            applied,
        };
        let s = state(w).unwrap();
        s.last_closed.insert(d.window.target, d.window.turn);
        if d.chance_percent.is_none() {
            s.pending.insert(d.identity.clone(), d.clone());
        }
        dr::trace(
            w,
            d.window.target,
            "14.1.2.automatic_gun_deck",
            json!({"identity":d.identity,"source_hit":d.window.hit.identity,"turn":d.window.turn,"bands":d.window.bands,"d100":d.d100,"chance_percent":d.chance_percent,"applied":d.applied,"source":tables()["source"],"execution":tables()["execution"]["revision"],"pending":d.chance_percent.is_none()}),
        );
        result.push(d);
    }
    result
}

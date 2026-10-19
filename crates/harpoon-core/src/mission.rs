//! Scenario-authored mission conditions, independent of weapon adjudication.
//! These are exercise objectives, not a new Harpoon combat rule.
use crate::geom::Vec2;
use crate::types::{EntityId, Side};
use crate::world::World;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AreaDenialConfig {
    pub defender: Side,
    pub center: Vec2,
    pub radius_nm: f64,
    pub hold_s: i64,
    pub functional_hp_fraction: f64,
    pub protected_names: Vec<String>,
    pub intruder_names: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MissionState {
    pub inside_since: BTreeMap<EntityId, i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MissionObservation {
    pub config: AreaDenialConfig,
    pub remaining_s: i64,
    pub own_functional: usize,
    pub own_total: usize,
    /// Defender never receives a hidden enemy's occupancy clock.
    pub own_hold_s: Option<i64>,
}

fn functional(world: &World, name: &str, threshold: f64) -> bool {
    world.entities.values().any(|e| {
        e.name == name && e.alive && e.hp_max > 0.0 && e.hp / e.hp_max > threshold
    })
}

pub fn observe(world: &World, side: Side) -> Option<MissionObservation> {
    let config = world.context.area_denial.as_ref()?;
    let own_names = if side == config.defender { &config.protected_names } else { &config.intruder_names };
    Some(MissionObservation {
        config: config.clone(),
        remaining_s: (world.duration_s - world.tick).max(0),
        own_functional: own_names.iter().filter(|name| functional(world, name, config.functional_hp_fraction)).count(),
        own_total: own_names.len(),
        own_hold_s: (side != config.defender).then(|| {
            world.mission_state.inside_since.values().map(|t| world.tick - t + 1).max().unwrap_or(0)
        }),
    })
}

/// Each named intruder must independently hold the circle continuously.
/// Leaving it, sinking, or reaching the exercise's damage threshold resets its clock.
pub fn adjudicate(world: &mut World) -> Option<(Option<Side>, String)> {
    let config = world.context.area_denial.as_ref()?.clone();
    let defender = config.defender;
    let attacker = defender.other();
    let defenders = config.protected_names.iter().filter(|n| functional(world, n, config.functional_hp_fraction)).count();
    let attackers = config.intruder_names.iter().filter(|n| functional(world, n, config.functional_hp_fraction)).count();
    if defenders == 0 && attackers == 0 {
        return Some((None, "mission_mutual_carrier_loss".into()));
    }
    if defenders == 0 { return Some((Some(attacker), "mission_defending_carriers_disabled".into())); }
    if attackers == 0 { return Some((Some(defender), "mission_intruding_carriers_disabled".into())); }
    let inside: Vec<EntityId> = world.entities.values().filter(|e| {
        e.side == attacker && config.intruder_names.contains(&e.name) && e.alive
            && e.hp_max > 0.0 && e.hp / e.hp_max > config.functional_hp_fraction
            && e.pos.range_to(config.center) <= config.radius_nm
    }).map(|e| e.id).collect();
    world.mission_state.inside_since.retain(|id, _| inside.contains(id));
    for id in inside { world.mission_state.inside_since.entry(id).or_insert(world.tick); }
    let held = world.mission_state.inside_since.values().any(|start| world.tick - start + 1 >= config.hold_s);
    if held { return Some((Some(attacker), "mission_zone_held".into())); }
    if world.tick >= world.duration_s {
        return if defenders == config.protected_names.len() {
            Some((Some(defender), "mission_area_denied".into()))
        } else {
            Some((None, "mission_denied_with_carrier_loss".into()))
        };
    }
    None
}

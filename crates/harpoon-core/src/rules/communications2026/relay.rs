//! §6.3.9 / Annex L Relay: hop-by-hop coverage extension through similarly
//! equipped platforms.
//!
//! Printed sources (personally viewed 2026-10-30):
//! - Annex L footnote, identical wording on USN 1 Dec 25 PDF94/printed L-1 and
//!   China's Navy 1 Dec 25 PDF96/printed K-3: "Relay: TDL coverage can be
//!   extended by using similarly equipped platforms to relay data to platforms
//!   over the horizon." Nothing is printed about hop counts, per-hop delay,
//!   refresh timing or probability.
//! - English2026 PDF74/6-6 §6.3.9: LOS surface/air scopes, beyond-LOS is the
//!   TDL's own HF radio (ES-detectable — never a relay), SATCOM where printed
//!   "Yes"; RT TDLs convey fire-control-quality solutions without degradation;
//!   receipt timing is the printed same-Detection-Phase / same-Increment rule
//!   already owned by the existing exchange consumers.
//! - The relay-enabled terminals are exactly the Annex L registry rows whose
//!   Beyond-LOS cell prints "Relay" (tdl-annex-2025.json `relay_printed`):
//!   us_link_16, cn_dts_03, cn_tjn_906. Link 11's beyond-LOS 300 is a numeric
//!   HF range, not relay; CEC and every other row never relays.
//!
//! Design: the fixed author peer graph is the EXISTING communications
//! Configuration (per-unit fits, directed `peers`, `peer_fits`); no reciprocal
//! edge is inferred and no enemy unit is ever considered. `find_path` is a
//! deterministic, read-only BFS that returns the fewest-hop unit list; every
//! hop must pass the existing `links::route` (compatibility, Annex L platform
//! binding and printed surface/air LOS caps, unmanned gate, CaS radio-pair
//! room gate, declared LOS transport range AND author antenna horizon). The
//! relay conveys the sender's original snapshot unchanged: it creates no
//! report, no FCS and never rewrites quality. Cross-terminal or
//! cross-protocol chains (e.g. Link16–Link11–Link16) are not printed and stay
//! pending; a Reagan-style binding without an exact DB class stays pending.
//!
//! Parent wiring (root only): `pub mod relay;` in communications2026.rs and
//! the delivery re-verification that consumes find_path. This file compiles
//! only after that declaration exists; nothing here mutates the World.

use super::*;
use std::collections::VecDeque;

pub fn tables() -> &'static Value {
    static TABLES: OnceLock<Value> = OnceLock::new();
    TABLES.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/tdl-relay-2026.json"
        ))
        .expect("tdl-relay-2026.json source/policy")
    })
}

/// Only a terminal whose Annex L row literally prints "Relay" in Beyond LOS
/// may extend coverage; HF numeric ranges and "--" never become relay.
pub(super) fn relay_enabled(annex_id: &str) -> Result<(), String> {
    let row = super::annex::tables()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"].as_str() == Some(annex_id))
        .ok_or_else(|| "Annex L unknown source terminal".to_string())?;
    if row["relay_printed"].as_bool() != Some(true) {
        return Err(format!(
            "Annex L {annex_id} does not print Relay in Beyond LOS; HF ranges and '--' never become relay"
        ));
    }
    Ok(())
}

/// Deterministic fewest-hop relay path [sender, ..., receiver] over the fixed
/// author peer graph, or an honest refusal. Read-only: no die, no state
/// change, no snapshot rewrite. Hop caps come from the printed Annex L row
/// through the existing route, never from this file.
pub fn find_path(w: &World, sender: EntityId, receiver: EntityId, fit_key: &str) -> Result<Vec<EntityId>, String> {
    Ok(find_path_with_fit(w, sender, receiver, fit_key)?.0)
}
pub(super) fn find_path_with_fit(w: &World, sender: EntityId, receiver: EntityId, fit_key: &str) -> Result<(Vec<EntityId>, String), String> {
    if sender == receiver { return Err(source_error()); }
    let state = w.rules.communications.as_ref().ok_or_else(source_error)?;
    check_state(w, state)?;
    let side = w.entity(sender).filter(|e| e.alive).ok_or_else(source_error)?.side;
    owned(w, side, receiver)?;
    let start = fit(w, &state.config, side, sender, fit_key)?;
    if !start.send || !start.operational || start.warning_kind.is_voice() { return Err(source_error()); }
    let annex_id = start.annex_system.as_ref().ok_or_else(source_error)?;
    relay_enabled(annex_id)?;
    let mut queue = VecDeque::from([(sender, fit_key.to_string(), vec![sender])]);
    let mut visited = BTreeSet::from([(sender, fit_key.to_string())]);
    while let Some((node, key, path)) = queue.pop_front() {
        let Ok(current) = fit(w, &state.config, side, node, &key) else { continue };
        if !current.send || !current.operational || current.annex_system.as_ref() != Some(annex_id) { continue; }
        let mut peers = current.peers.clone(); peers.sort(); peers.dedup();
        for name in peers {
            if !unique_own_name(w, side, &name) { continue; }
            let Some(peer) = w.entities.values().find(|e| e.side == side && e.alive && e.name == name) else { continue };
            let next_key = current.peer_fits.get(&name).cloned().unwrap_or_else(|| key.clone());
            let Ok(next) = fit(w, &state.config, side, peer.id, &next_key) else { continue };
            if next.warning_kind != start.warning_kind || next.annex_system.as_ref() != Some(annex_id) || !next.receive || !next.operational || (peer.id != receiver && !next.send) { continue; }
            if direct_edges(w, &state.config, side, node, &key, &[peer.id]).is_err() { continue; }
            let mut next_path = path.clone(); next_path.push(peer.id);
            if peer.id == receiver { return Ok((next_path, next_key)); }
            if visited.insert((peer.id, next_key.clone())) { queue.push_back((peer.id, next_key, next_path)); }
        }
    }
    Err("no qualified Annex L relay path".into())
}

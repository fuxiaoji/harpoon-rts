//! Opt-in real Annex L terminals; author topology and coverage remain separate.
//! Missing cells and Relay never turn into unlimited range or free equipment.
use super::*;

pub fn tables() -> &'static Value {
    static TABLES: OnceLock<Value> = OnceLock::new();
    TABLES.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../data/rules/tdl-annex-2025.json"
        ))
        .expect("visually verified Annex L registry")
    })
}
/// Additional class-exact printed platform bindings; included in Full source fingerprint.
pub fn extra_tables() -> &'static Value {
    static EXTRA: OnceLock<Value> = OnceLock::new();
    EXTRA.get_or_init(|| serde_json::from_str(include_str!("../../../../../data/rules/tdl-platform-extra-2025.json")).expect("verified Annex A platform bindings"))
}
#[derive(Deserialize)]
struct Registry {
    rows: Vec<Terminal>,
    platform_bindings: BTreeMap<String, PlatformFit>,
}
#[derive(Deserialize)]
struct PlatformFit {
    systems: Vec<String>,
}
#[derive(Deserialize)]
struct Terminal {
    id: String,
    warning_kind: LinkKind,
    surface_los_nm: Option<f64>,
    air_los_nm: Option<f64>,
    beyond_los_nm: Option<f64>,
    satcom_yes_printed: bool,
    receive_only: bool,
    dedicated_targeting_system: bool,
    compatible_systems: Vec<String>,
    single_ship_peer: bool,
}
fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY
        .get_or_init(|| serde_json::from_value(tables().clone()).expect("typed Annex L registry"))
}
fn terminal(f: &VoiceFit) -> Result<Option<&'static Terminal>, String> {
    f.annex_system
        .as_ref()
        .map(|key| {
            registry()
                .rows
                .iter()
                .find(|r| &r.id == key)
                .ok_or_else(|| "Annex L unknown source terminal".to_string())
        })
        .transpose()
}
pub(super) fn descriptor(f: &VoiceFit) -> Result<(), String> {
    let Some(row) = terminal(f)? else {
        return Ok(());
    };
    if f.protocol != row.id
        || f.warning_kind != row.warning_kind
        || (row.receive_only && f.send)
        || (f.targeting_system && !row.dedicated_targeting_system)
        || f.compatible_protocols
            .iter()
            .any(|p| !row.compatible_systems.contains(p))
    {
        return Err("Annex L type, protocol, receive-only, compatibility or targeting qualification differs from source".into());
    }
    match f.transport {
        Transport::AuthorDirected => {
            Err("Annex L real terminal requires a physical declared transport".into())
        }
        Transport::Hf { .. } if row.beyond_los_nm.is_none() => Err(
            "Annex L has no numeric HF range; Relay requires a qualified physical relay path"
                .into(),
        ),
        Transport::Satellite { .. } if !row.satcom_yes_printed => {
            Err("Annex L does not authorize SATCOM for this terminal".into())
        }
        _ => Ok(()),
    }
}
pub(super) fn platform(w: &World, unit: EntityId, f: &VoiceFit) -> Result<(), String> {
    let Some(row) = terminal(f)? else {
        return Ok(());
    };
    descriptor(f)?;
    let e = w.entity(unit).ok_or_else(source_error)?;
    let base = registry().platform_bindings.get(&e.class)
        .is_some_and(|p| p.systems.contains(&row.id));
    let extra = extra_tables()["platform_bindings"][&e.class]["systems"].as_array()
        .is_some_and(|systems| systems.iter().any(|system| system.as_str() == Some(&row.id)));
    if !base && !extra {
        return Err(
            "Annex L terminal has no verified equipment binding for this actual platform class"
                .into(),
        );
    }
    Ok(())
}
/// Validate actual bodies after scenario entities exist, not just class names in
/// a free-form per-unit config. Runtime routes repeat this after restoration.
pub(crate) fn initialize(w: &World) -> Result<(), String> {
    let Some(state) = w.rules.communications.as_ref() else {
        return Ok(());
    };
    for (name, u) in &state.config.units {
        for f in u.fits.values().filter(|f| f.annex_system.is_some()) {
            let mut matches = w.entities.values().filter(|e| &e.name == name);
            let e = matches
                .next()
                .ok_or("Annex L source-fitted unit is missing")?;
            if matches.next().is_some() {
                return Err("Annex L source-fitted unit name is ambiguous".into());
            }
            platform(w, e.id, f)?;
        }
    }
    Ok(())
}
pub(super) fn compatible(tx: &VoiceFit, rx: &VoiceFit) -> Result<bool, String> {
    match (terminal(tx)?, terminal(rx)?) {
        (None, None) => {
            Ok(tx.protocol == rx.protocol || tx.compatible_protocols.contains(&rx.protocol))
        }
        (Some(t), Some(r)) => Ok(t.id == r.id || t.compatible_systems.contains(&r.id)),
        _ => Err("Annex L real-to-authored gateway is source-pending".into()),
    }
}
pub(super) fn route(
    w: &World,
    a: EntityId,
    b: EntityId,
    tx: &VoiceFit,
    rx: &VoiceFit,
) -> Result<(), String> {
    let (Some(t), Some(r)) = (terminal(tx)?, terminal(rx)?) else {
        return if tx.annex_system.is_none() && rx.annex_system.is_none() {
            Ok(())
        } else {
            Err("Annex L real-to-authored gateway is source-pending".into())
        };
    };
    platform(w, a, tx)?;
    platform(w, b, rx)?;
    let ae = w.entity(a).ok_or_else(source_error)?;
    let be = w.entity(b).ok_or_else(source_error)?;
    let range = ae.pos.range_to(be.pos);
    let cap = |row: &Terminal, f: &VoiceFit| -> Result<f64, String> {
        match f.transport {
            Transport::LineOfSight { .. } => {
                if ae.kind == PlatformKind::Aircraft || be.kind == PlatformKind::Aircraft {
                    row.air_los_nm
                } else if ae.kind == PlatformKind::Ship && be.kind == PlatformKind::Ship {
                    row.surface_los_nm
                } else {
                    None
                }
            }
            Transport::Hf { .. } => row.beyond_los_nm,
            Transport::Satellite { available: true } if row.satcom_yes_printed => {
                return Ok(f64::MAX)
            }
            _ => None,
        }
        .ok_or_else(|| {
            "Annex L printed transport/range unavailable for these physical endpoints".into()
        })
    };
    if range > cap(t, tx)?.min(cap(r, rx)?) {
        return Err("Annex L printed endpoint range exceeded".into());
    }
    // Fixed author peer graph may not silently attach multiple ships to one
    // Hawklink endpoint. Dynamic switching has no command and remains pending.
    for (e, f, row) in [(ae, tx, t), (be, rx, r)] {
        if row.single_ship_peer {
            let mut ships: BTreeSet<EntityId> = w
                .entities
                .values()
                .filter(|p| {
                    p.side == e.side
                        && p.alive
                        && p.id != e.id
                        && p.kind == PlatformKind::Ship
                        && f.peers.contains(&p.name)
                })
                .map(|p| p.id)
                .collect();
            // Directed sender edges still consume the receiving terminal's
            // physical association. A receive-only peer list is not a loophole.
            if let Some(config) = w.rules.communications.as_ref().map(|s| &s.config) {
                if let Some(own) = config.units.get(&e.name) {
                    for peer in w.entities.values().filter(|p| {
                        p.side == e.side && p.alive && p.id != e.id && p.kind == PlatformKind::Ship
                    }) {
                        let Some(u) = config.units.get(&peer.name) else {
                            continue;
                        };
                        for (key, other) in &u.fits {
                            let receiver_key = other.peer_fits.get(&e.name).unwrap_or(key);
                            if other.send
                                && other.operational
                                && other.peers.contains(&e.name)
                                && other.annex_system.as_ref() == Some(&row.id)
                                && own
                                    .fits
                                    .get(receiver_key)
                                    .is_some_and(|r| r.annex_system.as_ref() == Some(&row.id))
                            {
                                ships.insert(peer.id);
                            }
                        }
                    }
                }
            }
            if ships.len() > 1 {
                return Err("Annex L Hawklink supports only one ship at a time".into());
            }
        }
    }
    Ok(())
}

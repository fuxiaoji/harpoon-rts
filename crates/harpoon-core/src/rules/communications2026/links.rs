use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    #[default]
    Voice,
    TimeLate,
    NearRealTime,
    RealTime,
}
impl LinkKind {
    pub fn is_voice(&self) -> bool {
        *self == Self::Voice
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transport {
    /// Explicit scenario-assumption connectivity, no claim of a physical range.
    #[default]
    AuthorDirected,
    LineOfSight {
        max_range_nm: f64,
        antenna_height_m: f64,
    },
    Hf {
        max_range_nm: f64,
    },
    Satellite {
        available: bool,
    },
}
impl Transport {
    pub fn is_author_directed(&self) -> bool {
        matches!(self, Self::AuthorDirected)
    }
}

/// Strongly-typed formation roster identity for the printed short-range radio
/// scope (expanded manual PDF14/15 row 94; English2026 PDF135/14-9 §14.5).
/// Never inferred from side, peer lists or proximity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FormationId(pub String);

/// Explicit scenario_assumption qualification of one fitted radio endpoint.
/// The printed communications groups name a radio horizon and a formation,
/// but the game prints neither an antenna-height table nor a formation
/// roster, so both are declared inputs. Nothing defaults and no real hardware
/// fit is claimed.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RadioQualification {
    /// Must be `scenario_assumption…`: no printed source supplies these values.
    pub source: String,
    /// Declared radio-horizon feed height (metres). Consumed only where the
    /// fitted transport prints no antenna height of its own (HF/satellite/
    /// author-directed): an HF antenna height is never assumed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub antenna_height_m: Option<f64>,
    /// Declared formation identity. Two ships fall inside the printed
    /// "same formation" scope only when both endpoints declare the same
    /// non-empty id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formation: Option<FormationId>,
}
impl RadioQualification {
    pub(super) fn validate(&self) -> Result<(), String> {
        if !authored(&self.source) {
            return Err(
                "6.3.9 radio qualification requires an explicit scenario_assumption source".into(),
            );
        }
        if let Some(h) = self.antenna_height_m {
            if !h.is_finite() || h < 0. {
                return Err(
                    "6.3.9 radio qualification antenna height must be finite and non-negative"
                        .into(),
                );
            }
        }
        if self.formation.as_ref().is_some_and(|f| f.0.trim().is_empty()) {
            return Err("6.3.9 radio qualification formation identity must be non-empty".into());
        }
        if self.antenna_height_m.is_none() && self.formation.is_none() {
            return Err(
                "6.3.9 radio qualification must declare an antenna height or a formation".into(),
            );
        }
        Ok(())
    }
    fn declared_height(&self) -> Option<f64> {
        self.antenna_height_m
    }
    fn declared_formation(&self) -> Option<&FormationId> {
        self.formation.as_ref()
    }
}

fn transport_shape(t: &Transport) -> u8 {
    match t {
        Transport::AuthorDirected => 0,
        Transport::LineOfSight { .. } => 1,
        Transport::Hf { .. } => 2,
        Transport::Satellite { .. } => 3,
    }
}

pub(super) fn validate_config(c: &Configuration) -> Result<(), String> {
    for u in c.units.values() {
        for f in u.fits.values() {
            super::annex::descriptor(f)?;
            if let Some(q) = &f.radio_qualification {
                q.validate()?;
            }
            match f.transport {
                Transport::LineOfSight {
                    max_range_nm,
                    antenna_height_m,
                } if !max_range_nm.is_finite()
                    || max_range_nm < 0.
                    || !antenna_height_m.is_finite()
                    || antenna_height_m < 0. =>
                {
                    return Err("6.3.9 invalid sourced LOS fit".into())
                }
                Transport::Hf { max_range_nm }
                    if !max_range_nm.is_finite() || max_range_nm < 0. =>
                {
                    return Err("6.3.9 invalid sourced HF range".into())
                }
                _ => {}
            }
        }
    }
    Ok(())
}
pub(super) fn effective(tx: &VoiceFit, rx: &VoiceFit) -> Result<LinkKind, String> {
    if tx.warning_kind.is_voice() != rx.warning_kind.is_voice() {
        return Err("6.3.9 voice and TDL do not establish a compatible data link".into());
    }
    Ok(tx.warning_kind.min(rx.warning_kind))
}
pub(super) fn route(
    w: &World,
    sender: EntityId,
    receiver: EntityId,
    tx: &VoiceFit,
    rx: &VoiceFit,
) -> Result<(), String> {
    effective(tx, rx)?;
    super::annex::route(w, sender, receiver, tx, rx)?;
    let a = w.entity(sender).ok_or_else(source_error)?;
    let b = w.entity(receiver).ok_or_else(source_error)?;
    for id in [sender, receiver] {
        if crate::rules::unmanned2026::is_configured(w, id)
            && !crate::rules::unmanned2026::allow_side_projection(w, id)
        {
            return Err("2.5 UUV modem/lost control cannot carry bulk TDL; use explicit single-contact acoustic exchange".into());
        }
    }
    let range = a.pos.range_to(b.pos);
    // Printed long-range scope: "units beyond the horizon". The horizon needs
    // both endpoint heights. An LOS transport prints its own antenna heights;
    // HF/satellite/author-directed transports qualify only through both
    // endpoints' declared qualification heights — an HF height is never assumed.
    let beyond_los = match (&tx.transport, &rx.transport) {
        (
            Transport::LineOfSight {
                antenna_height_m: ah,
                ..
            },
            Transport::LineOfSight {
                antenna_height_m: bh,
                ..
            },
        ) => Some(
            range > crate::geom::horizon_nm(a.altitude_m.max(0.) + ah, b.altitude_m.max(0.) + bh),
        ),
        (t, r) if transport_shape(t) == transport_shape(r) => {
            let ha = tx
                .radio_qualification
                .as_ref()
                .and_then(RadioQualification::declared_height);
            let hb = rx
                .radio_qualification
                .as_ref()
                .and_then(RadioQualification::declared_height);
            ha.zip(hb).map(|(ha, hb)| {
                range > crate::geom::horizon_nm(a.altitude_m.max(0.) + ha, b.altitude_m.max(0.) + hb)
            })
        }
        _ => None,
    };
    // Printed short-range scope: "other ships in the same formation". Only a
    // ship-ship pair can be inside it, and only through both endpoints'
    // declared formation ids; same side or a peer edge implies nothing.
    let same_formation = if a.kind == crate::types::PlatformKind::Ship
        && b.kind == crate::types::PlatformKind::Ship
    {
        match (
            tx.radio_qualification
                .as_ref()
                .and_then(RadioQualification::declared_formation),
            rx.radio_qualification
                .as_ref()
                .and_then(RadioQualification::declared_formation),
        ) {
            (Some(fa), Some(fb)) => Some(fa == fb),
            _ => None,
        }
    } else {
        Some(false)
    };
    crate::rules::expanded_damage2026::cas_communications::radio_pair_gate(
        w,
        sender,
        receiver,
        beyond_los,
        same_formation,
    )?;
    match (&tx.transport, &rx.transport) {
        (Transport::AuthorDirected, Transport::AuthorDirected) => Ok(()),
        (
            Transport::LineOfSight {
                max_range_nm: a_range,
                antenna_height_m: ah,
            },
            Transport::LineOfSight {
                max_range_nm: b_range,
                antenna_height_m: bh,
            },
        ) => {
            // Same radar-horizon geometry used by the existing source radar path;
            // antenna heights and equipment range must be explicitly authored.
            let horizon =
                crate::geom::horizon_nm(a.altitude_m.max(0.) + ah, b.altitude_m.max(0.) + bh);
            if !radio_exposed(w, a)
                || !radio_exposed(w, b)
                || range > a_range.min(*b_range).min(horizon)
            {
                Err("6.3.9 own units outside fitted LOS route".into())
            } else {
                Ok(())
            }
        }
        (Transport::Hf { max_range_nm: ar }, Transport::Hf { max_range_nm: br })
            if radio_exposed(w, a) && radio_exposed(w, b) && range <= ar.min(*br) =>
        {
            Ok(())
        }
        (Transport::Satellite { available: true }, Transport::Satellite { available: true })
            if radio_exposed(w, a) && radio_exposed(w, b) =>
        {
            Ok(())
        }
        _ => Err("6.3.9 compatible operational transport unavailable".into()),
    }
}
fn radio_exposed(w: &World, e: &crate::world::Entity) -> bool {
    e.depth_m == 0.
        || (e.depth_m <= 25.
            && ((w
                .rules
                .detection
                .as_ref()
                .and_then(|d| d.masts.get(&e.id))
                .is_some_and(|m| m.count > 0))
                || crate::rules::unmanned2026::is_configured(w, e.id)
                    && crate::rules::unmanned2026::radio_ready(w, e.id)))
}

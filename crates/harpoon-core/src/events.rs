//! Simulation events with per-side visibility — the double-blind referee's
//! dispatch table. Clients only ever receive `All` events plus events for
//! their own side; `Referee` events stay server-side.

use crate::geom::Vec2;
use crate::types::{ContactId, EntityId, Side};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    All,
    Side(Side),
    Referee,
}

impl Visibility {
    pub fn visible_to(self, side: Side) -> bool {
        match self {
            Visibility::All => true,
            Visibility::Side(s) => s == side,
            Visibility::Referee => false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub tick: i64,
    #[serde(flatten)]
    pub kind: EventKind,
    pub vis: Visibility,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum EventKind {
    ScenarioStarted { name: String },

    // --- detection / knowledge ---
    Detection {
        side: Side,
        contact: ContactId,
        category: String,
        pos_hint: Vec2,
        sensor: String,
    },
    ContactIdentified {
        side: Side,
        contact: ContactId,
        class_name: String,
    },
    ContactLost {
        side: Side,
        contact: ContactId,
    },
    ContactSank {
        side: Side,
        contact: ContactId,
    },
    CounterDetected {
        /// Side whose unit was revealed by its own emissions.
        side: Side,
        unit: EntityId,
        by_side: Side,
    },

    // --- weapons ---
    WeaponsRelease {
        side: Side,
        shooter: EntityId,
        shooter_name: String,
        weapon: String,
        count: u8,
    },
    Incoming {
        /// Defender side.
        side: Side,
        contact: ContactId,
        eta_s: f64,
    },
    SamShot {
        side: Side,
        shooter: EntityId,
        weapon: String,
        target_contact: ContactId,
    },
    Intercept {
        side: Side,
        contact: Option<ContactId>,
        intercepted: bool,
    },
    ChaffDeployed {
        side: Side,
        unit: EntityId,
    },
    DecoyFooled {
        side: Side,
        contact: Option<ContactId>,
    },
    GunAction {
        side: Side,
        shooter: EntityId,
        shooter_name: String,
        target_contact: ContactId,
        hits: u8,
    },
    TorpedoDetected {
        side: Side,
        contact: ContactId,
    },

    // --- damage ---
    Impact {
        side: Side,
        unit: EntityId,
        unit_name: String,
        weapon: String,
        damage: f64,
        hp_left: f64,
    },
    CriticalHit {
        side: Side,
        unit: EntityId,
        system: String,
    },
    Sunk {
        side: Side,
        unit: EntityId,
        unit_name: String,
    },
    Ditched {
        side: Side,
        unit: EntityId,
        unit_name: String,
    },
    AirShotDown {
        side: Side,
        unit: EntityId,
        unit_name: String,
        by_side: Side,
    },

    // --- air operations ---
    AirLaunch {
        side: Side,
        carrier: EntityId,
        class: String,
        count: u8,
        mission: String,
    },
    AirRecovered {
        side: Side,
        carrier: EntityId,
        class: String,
        count: u8,
    },
    DeckBusy {
        side: Side,
        carrier: EntityId,
    },

    // --- strike / bases ---
    BaseDamaged {
        side: Side,
        base: EntityId,
        base_name: String,
        damage: f64,
        hp_left: f64,
    },
    BaseDestroyed {
        side: Side,
        base: EntityId,
        base_name: String,
    },

    // --- victory ---
    ScenarioEnded {
        winner: Option<Side>,
        reason: String,
        vp_blue: f64,
        vp_red: f64,
    },
}

impl EventKind {
    pub fn name(&self) -> &'static str {
        match self {
            EventKind::ScenarioStarted { .. } => "scenario_started",
            EventKind::Detection { .. } => "detection",
            EventKind::ContactIdentified { .. } => "contact_identified",
            EventKind::ContactLost { .. } => "contact_lost",
            EventKind::ContactSank { .. } => "contact_sank",
            EventKind::CounterDetected { .. } => "counter_detected",
            EventKind::WeaponsRelease { .. } => "weapons_release",
            EventKind::Incoming { .. } => "incoming",
            EventKind::SamShot { .. } => "sam_shot",
            EventKind::Intercept { .. } => "intercept",
            EventKind::ChaffDeployed { .. } => "chaff_deployed",
            EventKind::DecoyFooled { .. } => "decoy_fooled",
            EventKind::GunAction { .. } => "gun_action",
            EventKind::TorpedoDetected { .. } => "torpedo_detected",
            EventKind::Impact { .. } => "impact",
            EventKind::CriticalHit { .. } => "critical_hit",
            EventKind::Sunk { .. } => "sunk",
            EventKind::Ditched { .. } => "ditched",
            EventKind::AirShotDown { .. } => "air_shot_down",
            EventKind::AirLaunch { .. } => "air_launch",
            EventKind::AirRecovered { .. } => "air_recovered",
            EventKind::DeckBusy { .. } => "deck_busy",
            EventKind::BaseDamaged { .. } => "base_damaged",
            EventKind::BaseDestroyed { .. } => "base_destroyed",
            EventKind::ScenarioEnded { .. } => "scenario_ended",
        }
    }
}

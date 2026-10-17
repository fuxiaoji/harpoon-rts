//! Player command set. All gameplay input funnels through this enum, which
//! makes replays trivial: `seed + scenario + command log == identical world`.

use crate::geom::Vec2;
use crate::types::{ContactId, EntityId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    SetCourse { unit: EntityId, course_deg: f64 },
    SetSpeed { unit: EntityId, speed_kts: f64 },
    SetDepth { unit: EntityId, depth_m: f64 },
    SetAltitude { unit: EntityId, altitude_m: f64 },
    Goto { unit: EntityId, waypoints: Vec<Vec2> },
    Stop { unit: EntityId },
    /// Formation station relative to a leader, expressed as
    /// (bearing offset from leader's bow, distance).
    Follow {
        unit: EntityId,
        leader: EntityId,
        bearing_rel_deg: f64,
        distance_nm: f64,
    },
    ClearStation { unit: EntityId },
    SetSonar { unit: EntityId, active: bool },
    SetWeaponsFree { unit: EntityId, free: bool },
    SetAutoChaff { unit: EntityId, on: bool },
    AssignTarget { unit: EntityId, contact: ContactId },
    ClearTarget { unit: EntityId },
    Fire {
        unit: EntityId,
        contact: ContactId,
        weapon: String,
        rounds: u8,
    },
    DeployChaff { unit: EntityId },
    LaunchAircraft {
        unit: EntityId,
        class: String,
        count: u8,
        mission: AirMission,
    },
    RecoverAircraft { unit: EntityId },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "mission", rename_all = "snake_case")]
pub enum AirMission {
    /// Combat air patrol around a station.
    Cap { station: Vec2, radius_nm: f64 },
    /// Strike a contact (engine resolves to truth; aircraft fly to contact
    /// estimate and release standoff weapons).
    Strike { contact: ContactId },
    /// ASW sweep with dipping sonar / light torpedoes.
    Asw { area: Vec2, radius_nm: f64 },
    /// Fly to a point and hold (recon/screening).
    Recon { point: Vec2 },
    /// Return to base/carrier.
    Rtb,
}

impl AirMission {
    pub fn tag(&self) -> &'static str {
        match self {
            AirMission::Cap { .. } => "cap",
            AirMission::Strike { .. } => "strike",
            AirMission::Asw { .. } => "asw",
            AirMission::Recon { .. } => "recon",
            AirMission::Rtb => "rtb",
        }
    }
}

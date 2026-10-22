//! Player command set. All gameplay input funnels through this enum, which
//! makes replays trivial: `seed + scenario + command log == identical world`.

use crate::geom::Vec2;
use crate::types::{ContactId, EntityId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Player authority over higher-level AI; current standing tasks/self-defence continue.
    SetUnitAutomation {unit:EntityId,enabled:bool},
    SetCourse { unit: EntityId, course_deg: f64 },
    SetSpeed { unit: EntityId, speed_kts: f64 },
    SetDepth { unit: EntityId, depth_m: f64 },
    SetAltitude { unit: EntityId, altitude_m: f64 },
    SetThrottle {unit: EntityId, setting:String},
    /// §14.4/14.5: deliberate damage-control decisions with persistent costs.
    DamageControl {unit:EntityId,action:String},
    SetTorpedoRun {unit:EntityId,weapon:String,deep:bool},
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
    /// Search-radar emission control; passive ESM/visual sensors stay available.
    SetRadar { unit: EntityId, active: bool },
    SetJamming {unit: EntityId, mode:String, #[serde(default)] contacts:Vec<ContactId>},
    /// Automatic-fire doctrine, not a weapon performance change. Manual Fire
    /// remains deliberate. Zero max_in_flight leaves legacy allocation intact.
    SetEngagementPolicy { unit: EntityId, air_only: bool, max_in_flight: u8 },
    /// Replace the standing flight task explicitly. Navigation orders alone do
    /// not replace an ASW/CAP autopilot. `manual` relinquishes flight steering.
    RetaskAircraft { unit: EntityId, mission: AirMission, #[serde(default)] manual: bool },
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
        /// Optional launch-time plotted orders (§4 opening paragraph). The
        /// old instantaneous RTS spawn profile remains the default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<LaunchProfile>,
    },
    RecoverAircraft { unit: EntityId },
    /// Individual recall; leaves other friendly patrols on their missions.
    ReturnToBase { unit: EntityId },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchProfile {
    /// Optional plotted departure orders; defaults retain old save behavior.
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub waypoints:Vec<Vec2>,
    pub altitude_m: f64,
    pub weapons_free: bool,
    /// Route and anti-ship release will be supplied as player commands.
    /// This selects flight automation, not a different combat rule.
    #[serde(default)]
    pub manual_strike: bool,
    /// Optional commander policy, applied atomically to every spawned aircraft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engagement: Option<LaunchEngagement>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchEngagement { pub air_only: bool, pub max_in_flight: u8 }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "mission", rename_all = "snake_case")]
pub enum AirMission {
    /// Combat air patrol around a station. With `follow`, the station tracks
    /// the launching carrier so the patrol escorts it as it moves.
    Cap { station: Vec2, radius_nm: f64, #[serde(default)] follow: bool },
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

//! Player command set. All gameplay input funnels through this enum, which
//! makes replays trivial: `seed + scenario + command log == identical world`.

use crate::geom::Vec2;
use crate::types::{ContactId, EntityId};
use serde::{Deserialize, Serialize};

#[path = "command_contract.rs"]
mod contract;
pub use contract::{command_schema, mission_schema};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Player authority over higher-level AI; current standing tasks/self-defence continue.
    SetUnitAutomation {unit:EntityId,enabled:bool},
    SetCourse { unit: EntityId, course_deg: f64 },
    SetSpeed { unit: EntityId, speed_kts: f64 },
    SetDepth { unit: EntityId, depth_m: f64 },
    SetAltitude { unit: EntityId, altitude_m: f64 },
    SetThrottle {unit: EntityId, setting:String},
    /// Explicit source-backed deck, flight and refuelling procedures (v4).
    Aviation {unit:EntityId,action:crate::rules::aviation2026::Action},
    AirCombat {unit:EntityId,action:crate::rules::aircombat2026::Action},
    SetArray {unit:EntityId,sensor:String,deploy:bool,depth_m:f64},
    Mines {unit:EntityId,action:crate::rules::mines2026::Action},
    Ground {unit:EntityId,action:crate::rules::ground2026::Action},
    SearchAndRescue {unit:EntityId,action:crate::rules::expanded_damage2026::RescueAction,
        #[serde(default,skip_serializing_if="omit_false")] retain_manual:bool},
    Detection {unit:EntityId,action:crate::rules::detection2026::DetectionAction},
    ElectronicWarfare {unit:EntityId,action:crate::rules::ew2026::Action},
    Torpedo {unit:EntityId,action:crate::rules::torpedo2026::Action},
    Asw {unit:EntityId,action:crate::rules::asw_weapons2026::Action,
        #[serde(default,skip_serializing_if="omit_false")] retain_manual:bool},
    Navigation {unit:EntityId,action:crate::rules::navigation2026::Action},
    Antiship {unit:EntityId,action:crate::rules::surface2026::antiship::Action},
    /// The player transport sets retain_manual; core commits it only if the
    /// source warning is accepted. AI inputs cannot acquire manual authority.
    Communications {unit:EntityId,action:crate::rules::communications2026::Action,
        #[serde(default,skip_serializing_if="omit_false")] retain_manual:bool},
    Unmanned {unit:EntityId,action:crate::rules::unmanned2026::Action,
        #[serde(default,skip_serializing_if="omit_false")] retain_manual:bool},
    Nuclear {unit:EntityId,action:crate::rules::nuclear2026::Action,
        #[serde(default,skip_serializing_if="omit_false")] retain_manual:bool},
    AamHoj {unit:EntityId,weapon:String,enabled:bool},
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

impl Command {
    /// Player intent is attached by a trusted transport, then committed with
    /// successful source application. It is never a public command-schema field.
    pub fn manual_intent(&self)->Option<bool>{match self {Self::Communications{retain_manual,..}|Self::Unmanned{retain_manual,..}|Self::Nuclear{retain_manual,..}|Self::SearchAndRescue{retain_manual,..}|Self::Asw{retain_manual,..}=>Some(*retain_manual),_=>None}}
    pub fn mark_player_intent(&mut self)->bool{match self {Self::Communications{retain_manual,..}|Self::Unmanned{retain_manual,..}|Self::Nuclear{retain_manual,..}|Self::SearchAndRescue{retain_manual,..}|Self::Asw{retain_manual,..}=>{*retain_manual=true;true},_=>false}}
    pub fn validate_source(&self,w:&crate::World,side:crate::Side)->Result<(),String>{
        let id=self.unit_id();
        if !matches!(self,Self::Unmanned{..}|Self::SetUnitAutomation{..}) {
            crate::rules::unmanned2026::can_control(w,side,id)?;
            if crate::rules::unmanned2026::basic_control_only(w,id)&&!matches!(self,Self::SetCourse{..}|Self::SetSpeed{..}|Self::SetDepth{..}|Self::Stop{..}) {return Err("2.5 acoustic modem carries limited basic steering/data only".into())}
        }
        match self {
            Self::Aviation{action:crate::rules::aviation2026::Action::WaterTakeoff,..}
            |Self::Aviation{action:crate::rules::aviation2026::Action::WaterLand,..}=>{
                if w.over||crate::exercise::locked(w,side)||!w.entity(id).is_some_and(|e|e.alive&&e.side==side){return Err("water operation requires an active own declared aircraft".into());}
                crate::rules::aviation2026::waterborne::validate(w,id,if matches!(self,Self::Aviation{action:crate::rules::aviation2026::Action::WaterTakeoff,..}){"takeoff"}else{"land"})
            },
            Self::SetSpeed{speed_kts,..} if crate::rules::aviation2026::waterborne::controls_water_motion(w,id)=>{
                if *speed_kts==0.{Ok(())}else{Err("4.6.5 resting aircraft requires declared water takeoff".into())}
            },
            Self::SetAltitude{altitude_m,..} if crate::rules::aviation2026::waterborne::controls_water_motion(w,id)=>{
                if *altitude_m==0.{Ok(())}else{Err("4.6.5 resting aircraft requires declared water takeoff".into())}
            },
            Self::ElectronicWarfare{action,..} if matches!(action,crate::rules::ew2026::Action::MobileSpeed{..}|crate::rules::ew2026::Action::FloatingTow{..}|crate::rules::ew2026::Action::FloatingUntow{..}|crate::rules::ew2026::Action::StartChaffBarrier{..}|crate::rules::ew2026::Action::StopChaffBarrier)=>{
                if w.over || crate::exercise::locked(w,side) || !w.entity(id).is_some_and(|e|e.alive&&e.side==side){return Err("EW order requires an active own fitted platform".into());}
                crate::rules::ew2026::validate_action(w,id,action)
            },
            Self::Torpedo{action:action @ crate::rules::torpedo2026::Action::Plot{..},..}=>{
                if w.over || crate::exercise::locked(w,side){return Err("torpedo Plot requires an active own platform".into());}
                crate::rules::torpedo2026::validate_plot(w,side,id,action).map(|_|())
            },
            Self::Detection{action:crate::rules::detection2026::DetectionAction::FireStarshell{contact,weapon},..}=>{
                if w.over || crate::exercise::locked(w,side) || !w.entity(id).is_some_and(|e|e.alive&&e.side==side) {return Err("5.8.8 active own gun required".into());}
                crate::rules::surface2026::gunnery::validate_starshell(w,side,id,*contact,weapon)
            },
            Self::Detection{action:crate::rules::detection2026::DetectionAction::EchoRange{initial_buoy},..}=>{
                if w.over || crate::exercise::locked(w,side) || !w.entity(id).is_some_and(|e|e.alive&&e.side==side) {return Err("echo ranging requires active own aircraft".into());}
                crate::rules::detection2026::sonobuoys::validate_action(w,id,&crate::rules::detection2026::sonobuoys::Action::EchoRange{initial_buoy:*initial_buoy})
            },
            Self::Detection{action:crate::rules::detection2026::DetectionAction::DropSonobuoy{model,depth_m,radio_channel},..}=>{
                if w.over || crate::exercise::locked(w,side) || !w.entity(id).is_some_and(|e|e.alive&&e.side==side) {
                    return Err("sonobuoy release requires an active own fitted platform".into());
                }
                crate::rules::detection2026::sonobuoys::validate_action(w,id,&crate::rules::detection2026::sonobuoys::Action::DropSonobuoy{model:model.clone(),depth_m:*depth_m,radio_channel:*radio_channel})
            },
            Self::Detection{action:crate::rules::detection2026::DetectionAction::ListenSonobuoys{buoys,purpose},..}=>{
                if w.over || crate::exercise::locked(w,side) || !w.entity(id).is_some_and(|e|e.alive&&e.side==side) {
                    return Err("sonobuoy monitoring requires an active own fitted platform".into());
                }
                crate::rules::detection2026::sonobuoys::validate_action(w,id,&crate::rules::detection2026::sonobuoys::Action::ListenSonobuoys{buoys:buoys.clone(),purpose:*purpose})
            },
            Self::Detection{action:crate::rules::detection2026::DetectionAction::Laser{action},..}=>{
                if w.over || crate::exercise::locked(w,side)
                    || !w.entity(id).is_some_and(|e|e.alive&&e.side==side) {
                    return Err("laser emission requires an active own fitted platform".into());
                }
                crate::rules::detection2026::laser::validate_action(w,id,action)
            },
            Self::Ground{action:action @ crate::rules::ground2026::Action::PumpDock{..},..}=>{
                if !w.entity(id).is_some_and(|e|e.alive&&e.side==side){return Err("drydock pumping requires own actual controller".into())}
                crate::rules::ground2026::drydock::validate_action(w,id,action)
            },
            Self::Ground{action:action @ crate::rules::ground2026::Action::GciVector{..},..}=>{
                if w.over || !w.entity(id).is_some_and(|e|e.alive&&e.side==side) || crate::exercise::locked(w,side) {return Err("GCI requires a live own controller in an active exercise".into())}
                crate::rules::ground2026::gci::validate_action(w,id,action)
            },
            Self::AirCombat{action:crate::rules::aircombat2026::Action::GuidedSurfaceAttack{weapon,rounds,briefing},..} => {
                crate::rules::pgm2026::validate_request(w,side,id,weapon,*rounds,briefing)
            },
            Self::Communications{action,..}=>crate::rules::communications2026::validate_action(w,side,id,action),
            Self::Unmanned{action,..}=>crate::rules::unmanned2026::validate_action(w,side,id,action),
            Self::Nuclear{action,..}=>crate::rules::nuclear2026::validate_action(w,side,id,action),
            Self::SearchAndRescue{action,..}=>crate::rules::expanded_damage2026::validate_rescue(w,id,action),
            Self::Asw{action,..}=>{
                if w.over || crate::exercise::locked(w,side) || !w.entity(id).is_some_and(|e|e.alive&&e.side==side){return Err("ASW pattern requires an active own unit".into())}
                crate::rules::asw_weapons2026::validate_action(w,id,action)
            },
            Self::Ground{action,..} if matches!(action,crate::rules::ground2026::Action::AlongsideResupply{..}|crate::rules::ground2026::Action::CancelAlongsideResupply{..})=>{
                if w.over || !w.entity(id).is_some_and(|e|e.alive&&e.side==side) || crate::exercise::locked(w,side) {return Err("alongside supply requires a live own recipient in an active exercise".into())}
                crate::rules::ground2026::berth::validate_action(w,id,action)
            },
            Self::Navigation{action:crate::rules::navigation2026::Action::Battery{action},..}=>{
                if w.over || crate::exercise::locked(w,side) || !w.entity(id).is_some_and(|e|e.alive&&e.side==side){return Err("battery order requires an active own submarine".into());}
                crate::rules::battery2026::validate_action(w,side,id,action)
            },
            Self::Navigation{action,..}=>crate::rules::navigation2026::validate_action(w,id,action),
            Self::SetDepth{depth_m,..}=>{
                crate::rules::battery2026::validate_depth_change(w,id,*depth_m)?;
                crate::rules::navigation2026::submarine::validate_depth_declaration(w,id,*depth_m)
            },
            Self::Fire{contact,weapon,rounds,..}=>{
                if crate::rules::aaw2026::active(w)
                    && w.db.weapons.get(weapon).is_some_and(|d|d.kind==crate::db::WeaponKind::Gun)
                    && w.contact(side,*contact).is_some_and(|c|crate::rules::detection2026::known_category(w,side,c.about)==Some(crate::types::PlatformKind::Ship)) {
                    crate::rules::surface2026::gunnery::validate(w,side,id,*contact,weapon,*rounds)?;
                }
                if crate::rules::ground2026::ngfs::configured(w,id,weapon)
                    && w.contact(side,*contact).is_some_and(|c|crate::rules::detection2026::known_category(w,side,c.about)==Some(crate::types::PlatformKind::Base)) {
                    if w.over || crate::exercise::locked(w,side) {return Err("11.4 requires an active exercise".into());}
                    crate::weapons::source_ngfs_ready(w,side,id,*contact,weapon,*rounds)?;
                }
                if w.db.weapons.get(weapon).is_some_and(crate::rules::strafe2026::source_weapon){return Err("9.6.9 declare the source aircraft gun pass through AirCombat, not generic ammunition Fire".into())}
                if w.db.weapons.get(weapon).is_some_and(|d|d.kind==crate::db::WeaponKind::Aam){crate::rules::aircombat2026::starting_weapon_gate(w,id,weapon)?;}
                if crate::rules::asw_weapons2026::is_depth_charge_configured(w,id,weapon){return Err("8.5.4 declare the complete physical depth-charge pattern through ASW, not generic Fire".into())}
                crate::rules::nuclear2026::release_gate(w,id,weapon)?;
                if !crate::rules::expanded_damage2026::weapon_operational(w,id,weapon)?{return Err("source physical weapon unavailable".into())}
                if crate::rules::asw_weapons2026::is_projector_configured(w,id,weapon) {
                    crate::rules::asw_weapons2026::validate_projector(w,id,*contact,weapon,u32::from(*rounds))?;
                } else if crate::rules::asw_weapons2026::active(w)&&w.db.weapons.get(weapon).is_some_and(|d|d.guidance==crate::db::Guidance::AswBallistic){crate::rules::asw_weapons2026::validate_fire(w,id,*contact,weapon)?;}
                Ok(())
            },
            _=>Ok(()),
        }
    }
    /// The issuing unit is shared by HTTP, MCP and automated controllers.
    /// A single exhaustive match prevents newer source commands losing their owner.
    pub fn unit_id(&self) -> EntityId {
        match self {
            Self::Asw{unit,..}|Self::Unmanned{unit,..}|Self::Nuclear{unit,..}|Self::SetUnitAutomation {unit,..}|Self::SetCourse {unit,..}|Self::SetSpeed {unit,..}|
            Self::SetDepth {unit,..}|Self::SetAltitude {unit,..}|Self::SetThrottle {unit,..}|Self::SearchAndRescue{unit,..}|
            Self::Aviation {unit,..}|Self::AirCombat {unit,..}|Self::SetArray {unit,..}|
            Self::Mines {unit,..}|Self::Ground {unit,..}|Self::Detection {unit,..}|
            Self::ElectronicWarfare {unit,..}|Self::Torpedo {unit,..}|Self::Navigation {unit,..}|Self::Antiship {unit,..}|Self::Communications {unit,..}|
            Self::AamHoj {unit,..}|Self::DamageControl {unit,..}|Self::SetTorpedoRun {unit,..}|
            Self::Goto {unit,..}|Self::Stop {unit}|Self::Follow {unit,..}|Self::ClearStation {unit}|
            Self::SetSonar {unit,..}|Self::SetRadar {unit,..}|Self::SetJamming {unit,..}|
            Self::SetEngagementPolicy {unit,..}|Self::RetaskAircraft {unit,..}|
            Self::SetWeaponsFree {unit,..}|Self::SetAutoChaff {unit,..}|Self::AssignTarget {unit,..}|
            Self::ClearTarget {unit}|Self::Fire {unit,..}|Self::DeployChaff {unit}|
            Self::LaunchAircraft {unit,..}|Self::RecoverAircraft {unit}|Self::ReturnToBase {unit} => *unit,
        }
    }
}

fn omit_false(value:&bool)->bool{!*value}

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

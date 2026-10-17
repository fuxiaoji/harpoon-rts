//! Data-driven platform / weapon / sensor database.
//!
//! The canonical set ships as embedded JSON (`db_data::BUILT_IN`) so the
//! engine has a single loadable, exportable source of truth. Scenarios may
//! layer overrides on top via `Database::merged`.

use crate::types::{PlatformKind, RcsClass};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeaponDef {
    pub id: String,
    pub name: String,
    pub kind: WeaponKind,
    pub targets: Targets,
    pub range_nm: f64,
    #[serde(default)]
    pub min_range_nm: f64,
    /// Projectile speed in knots (guns ignore this).
    pub speed_kts: f64,
    /// Torpedoes: distance at which the seeker goes live.
    #[serde(default)]
    pub enable_range_nm: f64,
    /// Base terminal hit probability per shot.
    pub pk: f64,
    /// Rounds launched per engagement.
    pub salvo: u8,
    /// Seconds between engagements per mount.
    pub reload_s: f64,
    pub damage: f64,
    pub guidance: Guidance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaponKind {
    Gun,
    Ciws,
    Sam,
    Aam,
    Ascm,
    Slcm,
    Torpedo,
    Lwt,
    AswRocket,
}

impl WeaponKind {
    pub fn is_gun_like(self) -> bool {
        matches!(self, WeaponKind::Gun | WeaponKind::Ciws)
    }

    pub fn is_air_defense(self) -> bool {
        matches!(self, WeaponKind::Sam | WeaponKind::Aam | WeaponKind::Ciws)
    }

    /// Kinds that fly out as a guided projectile entity.
    pub fn is_projectile(self) -> bool {
        !self.is_gun_like() && !matches!(self, WeaponKind::AswRocket)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Guidance {
    /// Gun-line: instant adjudication, no projectile.
    BallisticGun,
    /// Air-defense homing (SAM/AAM).
    HomingAir,
    /// Sea-skimming anti-ship / land-attack cruise missile.
    SeaSkimmer,
    /// Wake-homing torpedo.
    WakeHome,
    /// Active-homing torpedo.
    TorpedoActive,
    /// Ballistic ASW rocket: flies to a fixed aim point.
    AswBallistic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Targets {
    #[serde(default)]
    pub air: bool,
    #[serde(default)]
    pub surface: bool,
    #[serde(default)]
    pub sub: bool,
    #[serde(default)]
    pub missile: bool,
    #[serde(default)]
    pub land: bool,
}

impl Targets {
    pub fn accepts(&self, k: PlatformKind) -> bool {
        match k {
            PlatformKind::Ship => self.surface,
            PlatformKind::Sub => self.sub,
            PlatformKind::Aircraft => self.air,
            PlatformKind::Missile => self.missile,
            PlatformKind::Torpedo => false,
            PlatformKind::Base => self.land,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorDef {
    pub id: String,
    pub name: String,
    pub kind: SensorKind,
    #[serde(default)]
    pub air_range_nm: f64,
    #[serde(default)]
    pub surface_range_nm: f64,
    #[serde(default)]
    pub sub_range_nm: f64,
    #[serde(default)]
    pub missile_range_nm: f64,
    /// Mount height in meters; bounds radar/visual by the horizon.
    #[serde(default = "default_alt")]
    pub alt_m: f64,
}

fn default_alt() -> f64 {
    20.0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SensorKind {
    Visual,
    Radar,
    Esm,
    SonarPassive,
    SonarActive,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeaponFit {
    pub weapon: String,
    pub mounts: u8,
    #[serde(default = "default_ammo")]
    pub ammo: u16,
}

fn default_ammo() -> u16 {
    8
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeckDef {
    pub capacity: u8,
    /// Seconds between recovery and re-launch readiness.
    pub cycle_s: f64,
    pub wing: Vec<WingFit>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WingFit {
    pub class: String,
    pub count: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlatformClass {
    pub id: String,
    pub name: String,
    pub kind: PlatformKind,
    pub hp: f64,
    pub max_speed_kts: f64,
    #[serde(default)]
    pub cruise_speed_kts: f64,
    pub turn_rate_deg_s: f64,
    #[serde(default = "default_accel")]
    pub accel_kts_per_s: f64,
    pub rcs: RcsClass,
    /// Passive-sonar source level at reference speed.
    #[serde(default = "default_noise")]
    pub noise_db: f64,
    #[serde(default = "default_visual")]
    pub visual_factor: f64,
    pub vp_value: f64,
    #[serde(default)]
    pub sensors: Vec<String>,
    #[serde(default)]
    pub weapons: Vec<WeaponFit>,
    #[serde(default)]
    pub chaff: u16,
    #[serde(default)]
    pub decoys: u16,
    /// Aircraft only: airborne endurance in seconds.
    #[serde(default)]
    pub endurance_s: f64,
    #[serde(default)]
    pub max_depth_m: f64,
    /// Helicopters: can hover and operate a dipping sonar.
    #[serde(default)]
    pub dipping_sonar: bool,
    #[serde(default)]
    pub is_base: bool,
    #[serde(default)]
    pub deck: Option<DeckDef>,
}

fn default_accel() -> f64 {
    0.2
}
fn default_noise() -> f64 {
    50.0
}
fn default_visual() -> f64 {
    1.0
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Database {
    pub platforms: BTreeMap<String, PlatformClass>,
    pub weapons: BTreeMap<String, WeaponDef>,
    pub sensors: BTreeMap<String, SensorDef>,
}

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("missing {kind} definition: {id}")]
    Missing { kind: &'static str, id: String },
    #[error("invalid database: {0}")]
    Invalid(String),
}

impl Database {
    pub fn built_in() -> Database {
        let db: Database =
            serde_json::from_str(crate::db_data::BUILT_IN).expect("built-in database must parse");
        db.validate()
            .expect("built-in database must be internally consistent");
        db
    }

    pub fn from_json(json: &str) -> Result<Database, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Layer `overrides` on top of `self`, replacing whole entries per id.
    pub fn merged(&self, overrides: &Database) -> Database {
        let mut out = self.clone();
        for (k, v) in &overrides.platforms {
            out.platforms.insert(k.clone(), v.clone());
        }
        for (k, v) in &overrides.weapons {
            out.weapons.insert(k.clone(), v.clone());
        }
        for (k, v) in &overrides.sensors {
            out.sensors.insert(k.clone(), v.clone());
        }
        out
    }

    pub fn platform(&self, id: &str) -> Result<&PlatformClass, DbError> {
        self.platforms
            .get(id)
            .ok_or_else(|| DbError::Missing { kind: "platform", id: id.into() })
    }

    pub fn weapon(&self, id: &str) -> Result<&WeaponDef, DbError> {
        self.weapons
            .get(id)
            .ok_or_else(|| DbError::Missing { kind: "weapon", id: id.into() })
    }

    pub fn sensor(&self, id: &str) -> Result<&SensorDef, DbError> {
        self.sensors
            .get(id)
            .ok_or_else(|| DbError::Missing { kind: "sensor", id: id.into() })
    }

    /// Referential integrity check used at load time.
    pub fn validate(&self) -> Result<(), DbError> {
        for p in self.platforms.values() {
            for s in &p.sensors {
                self.sensor(s)?;
            }
            for w in &p.weapons {
                self.weapon(&w.weapon)?;
            }
            if let Some(deck) = &p.deck {
                if p.kind != PlatformKind::Ship && p.kind != PlatformKind::Base {
                    return Err(DbError::Invalid(format!(
                        "platform {} has a deck but is not a ship/base",
                        p.id
                    )));
                }
                for w in &deck.wing {
                    let ac = self.platform(&w.class)?;
                    if ac.kind != PlatformKind::Aircraft {
                        return Err(DbError::Invalid(format!(
                            "deck wing class {} is not an aircraft",
                            w.class
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

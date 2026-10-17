//! Shared identifiers and enumerations.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Blue,
    Red,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Blue => Side::Red,
            Side::Red => Side::Blue,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Side::Blue => 0,
            Side::Red => 1,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Side::Blue => "Blue",
            Side::Red => "Red",
        }
    }

    pub fn parse(s: &str) -> Option<Side> {
        match s.to_ascii_lowercase().as_str() {
            "blue" => Some(Side::Blue),
            "red" => Some(Side::Red),
            _ => None,
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u32);

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// A contact as perceived by one side. Ids are per-side, never shared across
/// sides and never map 1:1 onto entity ids from a client's point of view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ContactId(pub u32);

impl fmt::Display for ContactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "C{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformKind {
    Ship,
    Sub,
    Aircraft,
    Missile,
    Torpedo,
    Base,
}

impl PlatformKind {
    pub fn is_air(self) -> bool {
        matches!(self, PlatformKind::Aircraft)
    }

    pub fn is_projectile(self) -> bool {
        matches!(self, PlatformKind::Missile | PlatformKind::Torpedo)
    }

    /// Broad classification shown when a contact is first detected.
    pub fn category(self) -> &'static str {
        match self {
            PlatformKind::Ship => "Surface",
            PlatformKind::Sub => "Subsurface",
            PlatformKind::Aircraft => "Air",
            PlatformKind::Missile => "Missile",
            PlatformKind::Torpedo => "Torpedo",
            PlatformKind::Base => "Land",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RcsClass {
    Large,
    Medium,
    Small,
    VerySmall,
}

impl RcsClass {
    /// Relative radar cross-section multipliers (calibrated so that
    /// VerySmall/Large ≈ 0.2 per the official signature tables).
    pub fn factor(self) -> f64 {
        match self {
            RcsClass::Large => 1.0,
            RcsClass::Medium => 0.6,
            RcsClass::Small => 0.35,
            RcsClass::VerySmall => 0.2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DayNight {
    #[default]
    Day,
    Night,
}

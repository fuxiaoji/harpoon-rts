//! Expanded critical-hit tables (3-2) and per-location armor bands.
//!
//! Sources (`【20260405】鱼叉5（本体+中美俄数据库）`):
//! - `3-2.（扩展损伤手册26版）Harpoon_V_Expanded_Critical_Hit_Tables_(Chinese_Language).pdf`
//!   (Expanded Critical Hit Tables, Chinese ed. 1 Mar 2026):
//!   - p.2 general rules and the two formal changes. Relevant here: an
//!     airburst weapon critical never causes a magazine explosion, and a
//!     critical whose system is not fitted is ignored.
//!   - p.4 `1955年后水面战斗舰船（尺寸A至E）` — gun-hit column.
//!   - p.5 `制导武器对水面战斗舰船（尺寸A至E）` — guided-missile column.
//!   - p.8 `水下攻击` — torpedo / mine / collision column.
//!   - p.9 `空爆和弹片命中` — bomb / airburst & fragments column.
//! - `14-2.HpnV_Rules_1_Jan_26.pdf` PDF p.130 (printed 14-4) §14.1.6
//!   Effects of Armor: belt/deck armor, partial "critical-hit protection"
//!   armor, non-penetrating hits halve damage, and starred (*) criticals
//!   require the weapon to have penetrated. PDF pp.130-131 (printed
//!   14-4/14-5) §14.1.7: torpedo protection systems and the rule that an
//!   extreme bow or stern torpedo hit halves the damage.
//!
//! Roll convention: the engine draws D100 as `0..=99`. `CriticalRow.lo/hi`
//! are stored in the same 0-based space; the printed interval is in the
//! comment above each row (`01-20` = `0..=19`, `96-00` = `95..=99`).
//!
//! Scope: only the four surface-combatant columns named by the workstream are
//! transcribed here. The other 3-2 pages (pre-1955 ships, small craft,
//! aviation ships, merchants, transport craft, submarine heavy/light, and the
//! 《海上指挥》 tables) are not used by the engine and are deliberately left
//! out; submarine targets still roll the underwater column instead of the
//! p.12/p.13 submarine tables (known gap, see the workstream result notes).
//!
//! Legacy distribution note (refinement to the printed tables): before this
//! workstream the engine used a hand-made surface split of propulsion 17%,
//! radar 14%, sonar 10%, fire control 10%, weapons 10%, bridge 5%, rudder 5%,
//! CIC 5%, flooding 14%, fire 8%, magazine 2%, and an underwater split of
//! flooding 30%, propulsion 20%, hull penetration 10%, sonar 10%, fire
//! control 5%, CIC 5%, bridge 5%, rudder 5%, flooding 10%. The tables below
//! replace those percentages with the printed 3-2 bands; effect magnitudes
//! remain the engine's real-time adaptation, so the existing critical labels
//! and event shape are preserved.

use crate::db::WeaponKind;

/// What a critical row does to the target. Magnitudes are applied by
/// `crate::damage` (the real-time adaptation), not encoded in the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectSpec {
    /// Weapon mount / fire-control director; 3-2 p.4-p.9 `武器`.
    Weapon,
    /// One sensor or countermeasure; 3-2 `探测器`.
    Detector,
    /// Combat information centre; 3-2 `作战情报中心`.
    Cic,
    /// Engineering plant; 3-2 `轮机`.
    Propulsion,
    /// Hull flooding; 3-2 `进水`.
    Flooding,
    /// Fire casualty; 3-2 `火灾`.
    Fire,
    /// Bridge / conning station; 3-2 `舰桥`.
    Bridge,
    /// Rudder and steering; 3-2 `舵`.
    Rudder,
    /// Flight deck; only applicable to aviation ships (3-2 p.9 `飞行甲板`).
    FlightDeck,
    /// 3-2 p.9 D100 61-90: flight deck on aviation ships, weapon otherwise.
    WeaponOrFlightDeck,
}

/// One contiguous D100 band of one critical-hit column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CriticalRow {
    /// First engine roll covered (inclusive, 0-based).
    pub lo: u8,
    /// Last engine roll covered (inclusive, 0-based).
    pub hi: u8,
    /// Printed row name (English engine label; Chinese source quoted in docs).
    pub label: &'static str,
    /// Effect the row applies.
    pub effect: EffectSpec,
}

/// The four surface-combatant columns requested by the damage workstream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CriticalColumn {
    Gun,
    Missile,
    Torpedo,
    Bomb,
}

// ---------------------------------------------------------------------------
// 3-2 p.4 — `1955年后水面战斗舰船（尺寸A至E）` (gunfire column)
// Printed: 01-20 weapons, 21-35 detector, 36-45 CIC, 46-60 propulsion,
// 61-75 flooding, 76-90 fire, 91-95 bridge, 96-00 rudder. Total 100%.
// ---------------------------------------------------------------------------
pub const GUN_COLUMN: &[CriticalRow] = &[
    // D100 01-20: 武器 (weapons; directors are treated as mounts in-engine).
    CriticalRow { lo: 0, hi: 19, label: "weapon", effect: EffectSpec::Weapon },
    // D100 21-35: 探测器.
    CriticalRow { lo: 20, hi: 34, label: "detector", effect: EffectSpec::Detector },
    // D100 36-45: 作战情报中心.
    CriticalRow { lo: 35, hi: 44, label: "cic", effect: EffectSpec::Cic },
    // D100 46-60: 轮机*.
    CriticalRow { lo: 45, hi: 59, label: "propulsion", effect: EffectSpec::Propulsion },
    // D100 61-75: 进水*.
    CriticalRow { lo: 60, hi: 74, label: "flooding", effect: EffectSpec::Flooding },
    // D100 76-90: 火灾.
    CriticalRow { lo: 75, hi: 89, label: "fire", effect: EffectSpec::Fire },
    // D100 91-95: 舰桥.
    CriticalRow { lo: 90, hi: 94, label: "bridge", effect: EffectSpec::Bridge },
    // D100 96-00: 舵.
    CriticalRow { lo: 95, hi: 99, label: "rudder", effect: EffectSpec::Rudder },
];

// ---------------------------------------------------------------------------
// 3-2 p.5 — `制导武器对水面战斗舰船（尺寸A至E）` (guided-missile column)
// Printed: 01-10 weapons, 11-25 detector, 26-40 CIC, 41-65 propulsion,
// 66-80 flooding, 81-95 fire, 96-00 bridge. Total 100%.
// The p.5 note "a missile with a 10+ damage-point warhead also starts one
// automatic fire casualty" is NOT separately modelled by the engine: the
// engine's real-time critical probability already abstracts the missile's
// additional critical roll (see `damage.rs` module notes).
// ---------------------------------------------------------------------------
pub const MISSILE_COLUMN: &[CriticalRow] = &[
    // D100 01-10: 武器.
    CriticalRow { lo: 0, hi: 9, label: "weapon", effect: EffectSpec::Weapon },
    // D100 11-25: 探测器.
    CriticalRow { lo: 10, hi: 24, label: "detector", effect: EffectSpec::Detector },
    // D100 26-40: 作战情报中心.
    CriticalRow { lo: 25, hi: 39, label: "cic", effect: EffectSpec::Cic },
    // D100 41-65: 轮机*.
    CriticalRow { lo: 40, hi: 64, label: "propulsion", effect: EffectSpec::Propulsion },
    // D100 66-80: 进水*.
    CriticalRow { lo: 65, hi: 79, label: "flooding", effect: EffectSpec::Flooding },
    // D100 81-95: 火灾.
    CriticalRow { lo: 80, hi: 94, label: "fire", effect: EffectSpec::Fire },
    // D100 96-00: 舰桥.
    CriticalRow { lo: 95, hi: 99, label: "bridge", effect: EffectSpec::Bridge },
];

// ---------------------------------------------------------------------------
// 3-2 p.8 — `水下攻击` (torpedo / mine / collision column)
// Printed: 01-15 weapons, 16-25 detector, 26-30 CIC, 31-50 propulsion,
// 51-80 flooding, 81-90 fire, 91-95 bridge, 96-00 rudder. Total 100%.
// p.8 also lists automatic effects that are not part of the D100 table and
// are only partly modelled here (see `damage.rs` notes): every triggered
// torpedo/mine hit adds an automatic flooding critical; a wake-homer's first
// two criticals are rudder and propulsion; and a single torpedo causing 60%+
// of the original DP may break the keel (D10 1-8 = immediate sinking).
// ---------------------------------------------------------------------------
pub const TORPEDO_COLUMN: &[CriticalRow] = &[
    // D100 01-15: 武器.
    CriticalRow { lo: 0, hi: 14, label: "weapon", effect: EffectSpec::Weapon },
    // D100 16-25: 探测器.
    CriticalRow { lo: 15, hi: 24, label: "detector", effect: EffectSpec::Detector },
    // D100 26-30: 作战情报中心.
    CriticalRow { lo: 25, hi: 29, label: "cic", effect: EffectSpec::Cic },
    // D100 31-50: 轮机*.
    CriticalRow { lo: 30, hi: 49, label: "propulsion", effect: EffectSpec::Propulsion },
    // D100 51-80: 进水*.
    CriticalRow { lo: 50, hi: 79, label: "flooding", effect: EffectSpec::Flooding },
    // D100 81-90: 火灾.
    CriticalRow { lo: 80, hi: 89, label: "fire", effect: EffectSpec::Fire },
    // D100 91-95: 舰桥*/塔台.
    CriticalRow { lo: 90, hi: 94, label: "bridge", effect: EffectSpec::Bridge },
    // D100 96-00: 舵.
    CriticalRow { lo: 95, hi: 99, label: "rudder", effect: EffectSpec::Rudder },
];

// ---------------------------------------------------------------------------
// 3-2 p.9 — `空爆和弹片命中` (bomb / airburst column)
// Printed: 01-20 weapons*, 21-55 detector, 56-60 propulsion*, 61-90
// weapon/flight deck*, 91-95 flight deck*, 96-00 bridge*/tower. Total 100%.
// The p.2 formal change means a weapon critical on this column never rolls a
// magazine explosion (`damage::apply_effect` knows the column and skips it).
// ---------------------------------------------------------------------------
pub const BOMB_COLUMN: &[CriticalRow] = &[
    // D100 01-20: 武器*.
    CriticalRow { lo: 0, hi: 19, label: "weapon", effect: EffectSpec::Weapon },
    // D100 21-55: 探测器.
    CriticalRow { lo: 20, hi: 54, label: "detector", effect: EffectSpec::Detector },
    // D100 56-60: 轮机*.
    CriticalRow { lo: 55, hi: 59, label: "propulsion", effect: EffectSpec::Propulsion },
    // D100 61-90: 武器/飞行甲板* — flight deck on aviation ships, weapon
    // on every other ship.
    CriticalRow { lo: 60, hi: 89, label: "weapon_or_flight_deck", effect: EffectSpec::WeaponOrFlightDeck },
    // D100 91-95: 飞行甲板* (aviation ships only; ignored otherwise, per the
    // p.2 general rule for systems the ship does not carry).
    CriticalRow { lo: 90, hi: 94, label: "flight_deck", effect: EffectSpec::FlightDeck },
    // D100 96-00: 舰桥*/塔台.
    CriticalRow { lo: 95, hi: 99, label: "bridge", effect: EffectSpec::Bridge },
];

/// Rows of one critical column.
pub fn rows(column: CriticalColumn) -> &'static [CriticalRow] {
    match column {
        CriticalColumn::Gun => GUN_COLUMN,
        CriticalColumn::Missile => MISSILE_COLUMN,
        CriticalColumn::Torpedo => TORPEDO_COLUMN,
        CriticalColumn::Bomb => BOMB_COLUMN,
    }
}

/// Look up a row for an engine D100 roll in `0..=99`.
pub fn row_for_roll(column: CriticalColumn, roll: u8) -> Option<&'static CriticalRow> {
    if roll > 99 {
        return None;
    }
    rows(column).iter().find(|r| roll >= r.lo && roll <= r.hi)
}

/// Look up a row for a printed D100 result in `1..=100` (100 = printed `00`).
pub fn row_for_d100(column: CriticalColumn, d100: u8) -> Option<&'static CriticalRow> {
    match d100 {
        1..=100 => row_for_roll(column, d100 - 1),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Per-location armor bands (§14.1.6 / §14.1.7)
//
// The printed rules carry armor thickness in centimetres (e.g. Sverdlov
// 15/6, Nimitz 0/5) and a torpedo protection value in damage points; the
// engine has no armour/penetration fields, so this module stores damage
// multipliers instead. The numeric bands below are ESTIMATED adaptations
// requested by the damage workstream, not printed table values:
// - capital ship (cvn): bow 0.90 / side 1.00 / stern 0.80, torpedo
//   protection 0.75 (work-order bands);
// - escort (ddg_aegis): bow 0.95 / side 0.90 / stern 0.85, no torpedo-bulge
//   data (1.00) — the legacy 0.90 side value is preserved;
// - small craft (mgb, pb): unarmored, 1.00 everywhere;
// - classes without data: 1.00 everywhere (same "no data" fallback as the
//   previous damage_ext table, e.g. ffg).
// The 45°/135° bow/side/stern split is an engine estimate; §10-2 of the
// rules uses a target-aspect diagram rather than these exact cut-offs.
// ---------------------------------------------------------------------------

/// Damage multipliers for a class, by hit location.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoneArmor {
    pub bow: f64,
    pub side: f64,
    pub stern: f64,
    /// Fraction of an underwater hit left after the torpedo protection
    /// system / bulge (§14.1.7 PDF pp.130-131); 1.0 = no protection data.
    pub torpedo_protection: f64,
}

impl ZoneArmor {
    /// Unarmored / no data fallback.
    pub const NONE: ZoneArmor =
        ZoneArmor { bow: 1.0, side: 1.0, stern: 1.0, torpedo_protection: 1.0 };
}

/// Armor band for a platform class. Values are estimated (see module note).
pub fn armor_band(class: &str) -> ZoneArmor {
    match class {
        // Capital ship: bow 0.9 / side 1.0 / stern 0.8, torpedo 0.75.
        "cvn" => ZoneArmor { bow: 0.90, side: 1.00, stern: 0.80, torpedo_protection: 0.75 },
        // Escort tier: legacy side 0.90 preserved; bow/stern estimated.
        "ddg_aegis" => ZoneArmor { bow: 0.95, side: 0.90, stern: 0.85, torpedo_protection: 1.00 },
        // Small craft: unarmored.
        "mgb" | "pb" => ZoneArmor::NONE,
        // No data: no mitigation.
        _ => ZoneArmor::NONE,
    }
}

/// Which third of the ship a hit struck.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitZone {
    Bow,
    Side,
    Stern,
}

/// Classify the incoming bearing relative to the target's bow.
/// `None` means the bearing is unknown (e.g. an instant gun-line hit with no
/// shooter id); treat it as a broadside so the legacy behaviour is unchanged.
pub fn hit_zone(hit_bearing_rel_deg: Option<f64>) -> HitZone {
    match hit_bearing_rel_deg {
        None => HitZone::Side,
        Some(deg) => {
            // Normalise any angle into 0..=180 relative to the bow.
            let abs = crate::geom::angle_diff(0.0, deg).abs();
            if abs <= 45.0 {
                HitZone::Bow
            } else if abs >= 135.0 {
                HitZone::Stern
            } else {
                HitZone::Side
            }
        }
    }
}

/// Damage multiplier for a hit on `class` by `kind` at `hit_bearing_rel_deg`.
///
/// Surface weapons use the zone plate directly. Underwater weapons use
/// `2 - plate` (the legacy "torpedoes bypass belt armor" approximation, now
/// motivated by §14.1.7 PDF p.130: deep-running torpedoes hit below the belt)
/// times the torpedo-protection factor. An extreme bow/stern torpedo hit is
/// halved per §14.1.7 PDF p.130 ("极端舰首或舰尾命中会将大量能量耗费在推动
/// 海水上"), represented by `0.5` on top of the zone value.
pub fn armor_factor(class: &str, kind: WeaponKind, hit_bearing_rel_deg: Option<f64>) -> f64 {
    let band = armor_band(class);
    let zone = hit_zone(hit_bearing_rel_deg);
    let plate = match zone {
        HitZone::Bow => band.bow,
        HitZone::Side => band.side,
        HitZone::Stern => band.stern,
    };
    if matches!(kind, WeaponKind::Torpedo | WeaponKind::Lwt | WeaponKind::AswRocket) {
        let extreme = if zone == HitZone::Side { 1.0 } else { 0.5 };
        (2.0 - plate) * band.torpedo_protection * extreme
    } else {
        plate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_COLUMNS: [CriticalColumn; 4] = [
        CriticalColumn::Gun,
        CriticalColumn::Missile,
        CriticalColumn::Torpedo,
        CriticalColumn::Bomb,
    ];

    #[test]
    fn all_columns_cover_zero_to_ninety_nine_without_gaps() {
        for column in ALL_COLUMNS {
            let rows = rows(column);
            assert_eq!(rows.first().unwrap().lo, 0, "{column:?} must start at 0");
            assert_eq!(rows.last().unwrap().hi, 99, "{column:?} must end at 99");
            for pair in rows.windows(2) {
                assert_eq!(
                    pair[0].hi + 1,
                    pair[1].lo,
                    "{column:?} has a gap/overlap between {:?} and {:?}",
                    pair[0],
                    pair[1]
                );
            }
            for roll in 0..=99u8 {
                let hits: Vec<_> =
                    rows.iter().filter(|r| roll >= r.lo && roll <= r.hi).collect();
                assert_eq!(hits.len(), 1, "{column:?} roll {roll} matched {hits:?}");
                assert_eq!(row_for_roll(column, roll), Some(hits[0]));
            }
        }
    }

    #[test]
    fn printed_bands_map_to_the_expected_rows() {
        // Gun column (3-2 p.4).
        assert_eq!(row_for_d100(CriticalColumn::Gun, 1).unwrap().effect, EffectSpec::Weapon);
        assert_eq!(row_for_d100(CriticalColumn::Gun, 20).unwrap().effect, EffectSpec::Weapon);
        assert_eq!(row_for_d100(CriticalColumn::Gun, 21).unwrap().effect, EffectSpec::Detector);
        assert_eq!(row_for_d100(CriticalColumn::Gun, 36).unwrap().effect, EffectSpec::Cic);
        assert_eq!(row_for_d100(CriticalColumn::Gun, 46).unwrap().effect, EffectSpec::Propulsion);
        assert_eq!(row_for_d100(CriticalColumn::Gun, 61).unwrap().effect, EffectSpec::Flooding);
        assert_eq!(row_for_d100(CriticalColumn::Gun, 76).unwrap().effect, EffectSpec::Fire);
        assert_eq!(row_for_d100(CriticalColumn::Gun, 91).unwrap().effect, EffectSpec::Bridge);
        assert_eq!(row_for_d100(CriticalColumn::Gun, 96).unwrap().effect, EffectSpec::Rudder);
        assert_eq!(row_for_d100(CriticalColumn::Gun, 100).unwrap().label, "rudder");

        // Missile column (3-2 p.5).
        assert_eq!(row_for_d100(CriticalColumn::Missile, 10).unwrap().effect, EffectSpec::Weapon);
        assert_eq!(row_for_d100(CriticalColumn::Missile, 11).unwrap().effect, EffectSpec::Detector);
        assert_eq!(row_for_d100(CriticalColumn::Missile, 41).unwrap().effect, EffectSpec::Propulsion);
        assert_eq!(row_for_d100(CriticalColumn::Missile, 66).unwrap().effect, EffectSpec::Flooding);
        assert_eq!(row_for_d100(CriticalColumn::Missile, 81).unwrap().effect, EffectSpec::Fire);
        assert_eq!(row_for_d100(CriticalColumn::Missile, 96).unwrap().effect, EffectSpec::Bridge);

        // Torpedo column (3-2 p.8).
        assert_eq!(row_for_d100(CriticalColumn::Torpedo, 15).unwrap().effect, EffectSpec::Weapon);
        assert_eq!(row_for_d100(CriticalColumn::Torpedo, 26).unwrap().effect, EffectSpec::Cic);
        assert_eq!(row_for_d100(CriticalColumn::Torpedo, 31).unwrap().effect, EffectSpec::Propulsion);
        assert_eq!(row_for_d100(CriticalColumn::Torpedo, 51).unwrap().effect, EffectSpec::Flooding);
        assert_eq!(row_for_d100(CriticalColumn::Torpedo, 81).unwrap().effect, EffectSpec::Fire);
        assert_eq!(row_for_d100(CriticalColumn::Torpedo, 91).unwrap().effect, EffectSpec::Bridge);
        assert_eq!(row_for_d100(CriticalColumn::Torpedo, 96).unwrap().effect, EffectSpec::Rudder);

        // Bomb column (3-2 p.9).
        assert_eq!(row_for_d100(CriticalColumn::Bomb, 20).unwrap().effect, EffectSpec::Weapon);
        assert_eq!(row_for_d100(CriticalColumn::Bomb, 21).unwrap().effect, EffectSpec::Detector);
        assert_eq!(row_for_d100(CriticalColumn::Bomb, 56).unwrap().effect, EffectSpec::Propulsion);
        assert_eq!(
            row_for_d100(CriticalColumn::Bomb, 61).unwrap().effect,
            EffectSpec::WeaponOrFlightDeck
        );
        assert_eq!(row_for_d100(CriticalColumn::Bomb, 91).unwrap().effect, EffectSpec::FlightDeck);
        assert_eq!(row_for_d100(CriticalColumn::Bomb, 96).unwrap().effect, EffectSpec::Bridge);
    }

    #[test]
    fn each_column_totals_one_hundred_rolls() {
        for column in ALL_COLUMNS {
            let total: u32 =
                rows(column).iter().map(|r| (r.hi - r.lo + 1) as u32).sum();
            assert_eq!(total, 100, "{column:?}");
        }
    }

    #[test]
    fn zone_armor_bands_and_zones() {
        // Work-order bands (estimated): capital 0.9 / 1.0 / 0.8 with 0.75
        // torpedo protection; escort keeps the legacy 0.90 side.
        assert_eq!(armor_band("cvn").bow, 0.90);
        assert_eq!(armor_band("cvn").side, 1.00);
        assert_eq!(armor_band("cvn").stern, 0.80);
        assert_eq!(armor_band("cvn").torpedo_protection, 0.75);
        assert_eq!(armor_band("ddg_aegis").side, 0.90);
        assert_eq!(armor_band("mgb"), ZoneArmor::NONE);
        assert_eq!(armor_band("ffg"), ZoneArmor::NONE, "no data must not invent a plate");

        assert_eq!(hit_zone(None), HitZone::Side);
        assert_eq!(hit_zone(Some(0.0)), HitZone::Bow);
        assert_eq!(hit_zone(Some(45.0)), HitZone::Bow);
        assert_eq!(hit_zone(Some(45.1)), HitZone::Side);
        assert_eq!(hit_zone(Some(90.0)), HitZone::Side);
        assert_eq!(hit_zone(Some(134.9)), HitZone::Side);
        assert_eq!(hit_zone(Some(135.0)), HitZone::Stern);
        assert_eq!(hit_zone(Some(180.0)), HitZone::Stern);
        assert_eq!(hit_zone(Some(-30.0)), HitZone::Bow);
        assert_eq!(hit_zone(Some(210.0)), HitZone::Stern);

        let gun = WeaponKind::Gun;
        let torp = WeaponKind::Torpedo;
        assert_eq!(armor_factor("cvn", gun, Some(0.0)), 0.90);
        assert_eq!(armor_factor("cvn", gun, Some(90.0)), 1.00);
        assert_eq!(armor_factor("cvn", gun, Some(180.0)), 0.80);
        // (2 - 1.0) * 0.75 = 0.75 on the broadside.
        assert!((armor_factor("cvn", torp, Some(90.0)) - 0.75).abs() < 1e-12);
        // Extreme bow/stern underwater hits are halved (§14.1.7).
        assert!((armor_factor("cvn", torp, Some(0.0)) - 0.4125).abs() < 1e-12);
        assert!((armor_factor("ddg_aegis", torp, Some(90.0)) - 1.10).abs() < 1e-12);
        assert_eq!(armor_factor("mgb", gun, Some(0.0)), 1.0);
        assert_eq!(armor_factor("ffg", torp, Some(90.0)), 1.0);
    }
}

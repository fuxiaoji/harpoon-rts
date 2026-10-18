//! Fleet database layer — named real-world platforms (P1).
//!
//! This module embeds one JSON constant covering 26 ship/submarine classes
//! drawn from the official Admiralty Trilogy database volumes. The table
//! below is the required "simple chart" (class / source volume / printed page
//! / one-line note); printed page codes are the `A-n` folios in the volumes,
//! not PDF page numbers.
//!
//! ## Class chart
//!
//! | id | class | navy | source volume (1 Dec 25 printing) | page | note |
//! |---|---|---|---|---|---|
//! | cn_002 | Type 002 [Kuznetsov Mod] CV | PRC | China's Navy | A-3 | Shandong, 53,000 t, 1,020 DP |
//! | cn_039a | Type 039A/039B/039C [Yuan] SSP | PRC | China's Navy | A-5 | AIP, 3,600 t subm |
//! | cn_kilo | Pr. 636/636M [Improved Kilo] SS | PRC | China's Navy | A-6 | Chinese 636M, Klub-capable |
//! | cn_052d | Type 052D [Luyang III] DDG | PRC | China's Navy | A-8/A-9 | 64-cell VLS, 6,300 t |
//! | cn_055 | Type 055 [Renhai] CG | PRC | China's Navy | A-8 | 112-cell VLS, 10,300 t |
//! | cn_052c | Type 052C [Luyang II] DDG | PRC | China's Navy | A-9 | 48 HQ-9 cells, 6,200 t |
//! | cn_054a | Type 054A [Jiangkai II] FFG | PRC | China's Navy | A-12 | 32 HQ-16 cells |
//! | cn_056 | Type 056/056A [Jiangdao] FFL | PRC | China's Navy | A-16 | 1,300 t corvette |
//! | us_ford | Gerald R. Ford CVN | USA | America's Navy | A-4 | CVN-78, 1,667 DP |
//! | us_nimitz | Nimitz CVN | USA | America's Navy | A-5 | CVN-68, 1,605 DP |
//! | us_virginia | Virginia (ii) SSN | USA | America's Navy | A-13 | Blk I-IV, 168 DP |
//! | us_losangeles | Improved Los Angeles SSN | USA | America's Navy | A-14 | 688i, 159 DP |
//! | us_ticonderoga | Ticonderoga (VLS) CG | USA | America's Navy | A-25 | source entry "Bunker Hill CG", 295 DP |
//! | us_burke_2a | Arleigh Burke Flight IIA DDG | USA | America's Navy | A-31 | 96-cell VLS, 344 DP |
//! | ru_kuznetsov | Pr. 1143.5 Admiral Kuznetsov CV | Russia | Russia's Navy | A-4 | STOBAR carrier, 1,110 DP |
//! | ru_kilo | Pr. 636.3 Varshavyanka SS | Russia | Russia's Navy | A-20 | Improved Kilo, Kalibr |
//! | ru_slava | Pr. 1164 Atlant CG | Russia | Russia's Navy | A-25 | Slava/Moskva class |
//! | ru_kirov | Pr. 1144 Kirov CGN | Russia | Russia's Navy | A-25 | 20 P-700, 640 DP |
//! | ru_sovremenny | Pr. 956 Sarych DDG | Russia | Russia's Navy | A-30 | Moskit-armed |
//! | ru_udaloy | Pr. 1155 Udaloy DD | Russia | Russia's Navy | A-32/A-33 | ASW destroyer |
//! | uk_qe | Queen Elizabeth (ii) CV | UK | Commonwealth Navies | A-29 | 65,000 t fl, 1,052 DP |
//! | uk_vanguard | Vanguard SSBN | UK | Commonwealth Navies | A-33 | Trident SSBN |
//! | uk_astute | Astute SSN | UK | Commonwealth Navies | A-33 | 162 DP, Spearfish/Tomahawk |
//! | uk_type45 | Type 45 (Daring) DDG | UK | Commonwealth Navies | A-39 | Aster 15/30 |
//! | uk_type23 | Type 23 (Duke) FF | UK | Commonwealth Navies | A-44 | Sea Wolf/Harpoon |
//! | uk_type26 | Type 26 (City) FFG | UK | Commonwealth Navies | A-44 | Sea Ceptor, 2027+ |
//!
//! ## Calibration
//!
//! - `hp` is the last value of the volume's "Dam Pts" progression (the 100%
//!   row), i.e. the 2012 Standard Damage Point total. Example: Type 052D
//!   `0 51 102 153 184 204` → `hp: 204` (China's Navy A-9).
//! - `rcs` uses the first word of the volume's `Signature:` line:
//!   Large/Medium/Small/VSmall map to the four `RcsClass` bands.
//! - `noise_db` is an engine-scale conversion of the second signature word
//!   (the acoustic-generation band), anchored on the built-in database:
//!   Loud 66, Noisy 56-58, Quiet 44-46, VQuiet 26-34, EQuiet 22. The
//!   per-class value is interpolated by displacement within that band.
//!   These are adaptations, not volume numbers (the volumes publish
//!   signature classes, not dB).
//! - `max_speed_kts` is the first (undamaged) "Surf Speed"/"Subm Speed"
//!   value; `cruise_speed_kts` is a rule-of-thumb 60-70% of max for ships
//!   and the quiet patrol speed for submarines (estimated).
//! - Weapon fits are the volume's mounts with a typical loadout. `ammo` is
//!   per mount in `PlatformClass`; the scenario loader multiplies it by
//!   `mounts`, so the JSON ammo numbers below are already per-mount. VLS
//!   counts may exceed physical cells where the volume's loadout uses
//!   quad-packed ESSM; they are missile counts, not cell counts.
//! - Sensors use the existing engine sensor ids only (radar_air_long/med,
//!   radar_surf_med/sm, esm_std, sonar_p_hull/tow, sonar_a_hull, visual).
//!   Mapping: AESA/3D long-range air search (SPY-1/6, LJG-346, Sampson,
//!   SPS-48/49) → `radar_air_long`; medium 3D sets (Artisan, Fregat-MA,
//!   LJQ-382) → `radar_air_med`; surface/combined search → `radar_surf_med`
//!   or `radar_surf_sm`; hull sonar → `sonar_p_hull` + `sonar_a_hull`;
//!   towed/linear arrays → `sonar_p_tow`; all classes get `esm_std` and
//!   `visual` unless the volume says otherwise.
//! - Five new weapons are added in this module: `rasl_km` (the work order's
//!   long-range supersonic ASCM, i.e. the P-500/P-700/YJ-18/Moskit family),
//!   `sam_medium`, `gun_130mm`, `gun_100mm`, `gun_76mm`. Their values are
//!   estimates calibrated against the built-in gun/ASM/SAM entries; the
//!   supersonic ASM follows the work order's fixed speed/pk/damage
//!   (900 kt / 0.50 / 45). None of these ids exist in `db_data::BUILT_IN`.
//! - Gun mapping: 5in/4.5in mounts reuse `gun_5in`, 130 mm uses
//!   `gun_130mm`, 100 mm uses `gun_100mm`, 76 mm uses `gun_76mm`, and
//!   25/30/37 mm mounts reuse `gun_30mm`/`ciws_30mm`.
//! - `vp_value`, `turn_rate_deg_s`, `accel_kts_per_s`, chaff and decoy
//!   counts are engine-scale estimates derived from displacement, role and
//!   the volume's countermeasure fit; the volumes do not print them.
//!
//! ## Known simplifications
//!
//! - No SLBM weapon kind exists in the engine, so Vanguard's Trident D5
//!   tube is not fitted as a weapon (noted, not invented).
//! - The P-700/P-500/P-1000/YJ-18/Moskit family share one generic
//!   supersonic ASCM entry; seeker/range differences are not modelled.
//! - Type 26's Mk41 payload is not confirmed in the volume; only the
//!   confirmed 48 Sea Ceptor rounds and an estimated VL ASROC load are fitted.
//! - Carrier air wings are mapped onto the four built-in aircraft classes
//!   (fighter/striker/aew/helo_asw); the mapping is listed per deck below.

use crate::db::Database;

/// Every class in this layer. Used by the module's tests as a second
/// witness against accidental JSON edits.
#[cfg(test)]
const FLEET_IDS: [&str; 26] = [
    "cn_002",
    "cn_039a",
    "cn_kilo",
    "cn_052d",
    "cn_055",
    "cn_052c",
    "cn_054a",
    "cn_056",
    "us_ford",
    "us_nimitz",
    "us_virginia",
    "us_losangeles",
    "us_ticonderoga",
    "us_burke_2a",
    "ru_kuznetsov",
    "ru_kilo",
    "ru_slava",
    "ru_kirov",
    "ru_sovremenny",
    "ru_udaloy",
    "uk_qe",
    "uk_vanguard",
    "uk_astute",
    "uk_type45",
    "uk_type23",
    "uk_type26",
];

/// 100% damage-point totals transcribed from the volumes, used to catch
/// transcription slips. Page references are in the class chart above.
#[cfg(test)]
const KNOWN_DP: [(&str, f64); 26] = [
    ("cn_002", 1023.0),
    ("cn_039a", 107.0),
    ("cn_kilo", 91.0),
    ("cn_052d", 204.0),
    ("cn_055", 283.0),
    ("cn_052c", 201.0),
    ("cn_054a", 140.0),
    ("cn_056", 71.0),
    ("us_ford", 1667.0),
    ("us_nimitz", 1605.0),
    ("us_virginia", 168.0),
    ("us_losangeles", 159.0),
    ("us_ticonderoga", 295.0),
    ("us_burke_2a", 344.0),
    ("ru_kuznetsov", 1110.0),
    ("ru_kilo", 91.0),
    ("ru_slava", 284.0),
    ("ru_kirov", 640.0),
    ("ru_sovremenny", 223.0),
    ("ru_udaloy", 232.0),
    ("uk_qe", 1052.0),
    ("uk_vanguard", 271.0),
    ("uk_astute", 162.0),
    ("uk_type45", 246.0),
    ("uk_type23", 196.0),
    ("uk_type26", 263.0),
];

/// Embedded fleet data. Merged into `Database::built_in()` through
/// `crate::rules::database_extras()`.
pub fn db() -> Database {
    const FLEET_DB_JSON: &str = r#"{
  "platforms": {
    "cn_002": {
      "id": "cn_002", "name": "Type 002 [Kuznetsov Mod] CV", "kind": "Ship",
      "hp": 1023, "max_speed_kts": 29, "cruise_speed_kts": 18, "turn_rate_deg_s": 1.3,
      "rcs": "Large", "noise_db": 66, "vp_value": 70,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "sam_point", "mounts": 3, "ammo": 18},
        {"weapon": "ciws_30mm", "mounts": 3, "ammo": 30},
        {"weapon": "asw_rocket", "mounts": 2, "ammo": 6}
      ],
      "chaff": 60, "decoys": 14,
      "deck": {"capacity": 24, "cycle_s": 120, "wing": [
        {"class": "fighter", "count": 18},
        {"class": "helo_asw", "count": 6}
      ]}
    },
    "cn_039a": {
      "id": "cn_039a", "name": "Type 039A/039B/039C [Yuan] SSP", "kind": "Sub",
      "hp": 107, "max_speed_kts": 19, "cruise_speed_kts": 5, "turn_rate_deg_s": 2.5,
      "rcs": "VerySmall", "noise_db": 28, "visual_factor": 0.15, "vp_value": 28,
      "max_depth_m": 250,
      "sensors": ["sonar_p_hull", "sonar_a_hull", "esm_std", "visual"],
      "weapons": [
        {"weapon": "torp_homing", "mounts": 2, "ammo": 4},
        {"weapon": "ascm_std", "mounts": 1, "ammo": 4}
      ],
      "decoys": 10
    },
    "cn_kilo": {
      "id": "cn_kilo", "name": "Pr. 636/636M [Improved Kilo] SS", "kind": "Sub",
      "hp": 91, "max_speed_kts": 20, "cruise_speed_kts": 6, "turn_rate_deg_s": 2.5,
      "rcs": "VerySmall", "noise_db": 30, "visual_factor": 0.15, "vp_value": 22,
      "max_depth_m": 200,
      "sensors": ["sonar_p_hull", "sonar_a_hull", "esm_std", "visual"],
      "weapons": [
        {"weapon": "torp_wake", "mounts": 2, "ammo": 5},
        {"weapon": "torp_homing", "mounts": 1, "ammo": 6},
        {"weapon": "ascm_std", "mounts": 1, "ammo": 2}
      ],
      "decoys": 8
    },
    "cn_052d": {
      "id": "cn_052d", "name": "Type 052D [Luyang III] DDG", "kind": "Ship",
      "hp": 204, "max_speed_kts": 29, "cruise_speed_kts": 18, "turn_rate_deg_s": 2.0,
      "rcs": "Medium", "noise_db": 58, "vp_value": 32,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "sam_long", "mounts": 1, "ammo": 32},
        {"weapon": "ascm_std", "mounts": 2, "ammo": 8},
        {"weapon": "slcm", "mounts": 1, "ammo": 8},
        {"weapon": "asw_rocket", "mounts": 1, "ammo": 8},
        {"weapon": "sam_point", "mounts": 1, "ammo": 24},
        {"weapon": "gun_130mm", "mounts": 1, "ammo": 40},
        {"weapon": "gun_30mm", "mounts": 1, "ammo": 40},
        {"weapon": "lwt", "mounts": 2, "ammo": 6}
      ],
      "chaff": 64, "decoys": 16
    },
    "cn_055": {
      "id": "cn_055", "name": "Type 055 [Renhai] CG", "kind": "Ship",
      "hp": 283, "max_speed_kts": 31, "cruise_speed_kts": 18, "turn_rate_deg_s": 1.8,
      "rcs": "Small", "noise_db": 46, "vp_value": 42,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "sam_long", "mounts": 1, "ammo": 64},
        {"weapon": "ascm_std", "mounts": 2, "ammo": 12},
        {"weapon": "slcm", "mounts": 1, "ammo": 16},
        {"weapon": "asw_rocket", "mounts": 1, "ammo": 8},
        {"weapon": "sam_point", "mounts": 1, "ammo": 24},
        {"weapon": "gun_130mm", "mounts": 1, "ammo": 40},
        {"weapon": "ciws_30mm", "mounts": 1, "ammo": 30},
        {"weapon": "lwt", "mounts": 2, "ammo": 3}
      ],
      "chaff": 80, "decoys": 20
    },
    "cn_052c": {
      "id": "cn_052c", "name": "Type 052C [Luyang II] DDG", "kind": "Ship",
      "hp": 201, "max_speed_kts": 29, "cruise_speed_kts": 18, "turn_rate_deg_s": 2.0,
      "rcs": "Medium", "noise_db": 58, "vp_value": 30,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "sam_long", "mounts": 1, "ammo": 48},
        {"weapon": "ascm_std", "mounts": 2, "ammo": 4},
        {"weapon": "gun_100mm", "mounts": 1, "ammo": 40},
        {"weapon": "gun_30mm", "mounts": 2, "ammo": 40},
        {"weapon": "lwt", "mounts": 2, "ammo": 3}
      ],
      "chaff": 50, "decoys": 12
    },
    "cn_054a": {
      "id": "cn_054a", "name": "Type 054A [Jiangkai II] FFG", "kind": "Ship",
      "hp": 140, "max_speed_kts": 27, "cruise_speed_kts": 16, "turn_rate_deg_s": 2.4,
      "rcs": "Small", "noise_db": 56, "vp_value": 20,
      "sensors": ["radar_air_med", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "sam_medium", "mounts": 1, "ammo": 32},
        {"weapon": "ascm_std", "mounts": 2, "ammo": 4},
        {"weapon": "gun_76mm", "mounts": 1, "ammo": 40},
        {"weapon": "gun_30mm", "mounts": 2, "ammo": 40},
        {"weapon": "lwt", "mounts": 2, "ammo": 3},
        {"weapon": "asw_rocket", "mounts": 2, "ammo": 5}
      ],
      "chaff": 32, "decoys": 8
    },
    "cn_056": {
      "id": "cn_056", "name": "Type 056/056A [Jiangdao] FFL", "kind": "Ship",
      "hp": 71, "max_speed_kts": 28, "cruise_speed_kts": 16, "turn_rate_deg_s": 3.2,
      "rcs": "Small", "noise_db": 54, "vp_value": 8,
      "sensors": ["radar_surf_med", "radar_nav", "esm_std", "sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "ascm_std", "mounts": 2, "ammo": 1},
        {"weapon": "sam_point", "mounts": 1, "ammo": 8},
        {"weapon": "gun_76mm", "mounts": 1, "ammo": 40},
        {"weapon": "gun_30mm", "mounts": 2, "ammo": 20},
        {"weapon": "lwt", "mounts": 2, "ammo": 3}
      ],
      "chaff": 16, "decoys": 4
    },
    "us_ford": {
      "id": "us_ford", "name": "Gerald R. Ford CVN", "kind": "Ship",
      "hp": 1667, "max_speed_kts": 32, "cruise_speed_kts": 25, "turn_rate_deg_s": 1.2,
      "rcs": "Large", "noise_db": 66, "vp_value": 80,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "visual"],
      "weapons": [
        {"weapon": "sam_point", "mounts": 2, "ammo": 21},
        {"weapon": "sam_medium", "mounts": 2, "ammo": 4},
        {"weapon": "ciws_30mm", "mounts": 3, "ammo": 30},
        {"weapon": "gun_30mm", "mounts": 4, "ammo": 20}
      ],
      "chaff": 60, "decoys": 20,
      "deck": {"capacity": 40, "cycle_s": 90, "wing": [
        {"class": "fighter", "count": 22},
        {"class": "striker", "count": 12},
        {"class": "aew", "count": 4},
        {"class": "helo_asw", "count": 2}
      ]}
    },
    "us_nimitz": {
      "id": "us_nimitz", "name": "Nimitz CVN", "kind": "Ship",
      "hp": 1605, "max_speed_kts": 32, "cruise_speed_kts": 25, "turn_rate_deg_s": 1.2,
      "rcs": "Large", "noise_db": 66, "vp_value": 75,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "visual"],
      "weapons": [
        {"weapon": "sam_point", "mounts": 2, "ammo": 21},
        {"weapon": "ciws_30mm", "mounts": 3, "ammo": 30},
        {"weapon": "gun_30mm", "mounts": 2, "ammo": 20}
      ],
      "chaff": 60, "decoys": 20,
      "deck": {"capacity": 40, "cycle_s": 120, "wing": [
        {"class": "fighter", "count": 18},
        {"class": "striker", "count": 14},
        {"class": "aew", "count": 4},
        {"class": "helo_asw", "count": 4}
      ]}
    },
    "us_virginia": {
      "id": "us_virginia", "name": "Virginia (ii) SSN", "kind": "Sub",
      "hp": 168, "max_speed_kts": 34, "cruise_speed_kts": 10, "turn_rate_deg_s": 2.2,
      "rcs": "VerySmall", "noise_db": 22, "visual_factor": 0.12, "vp_value": 38,
      "max_depth_m": 450,
      "sensors": ["sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "esm_std", "visual"],
      "weapons": [
        {"weapon": "torp_homing", "mounts": 2, "ammo": 7},
        {"weapon": "slcm", "mounts": 1, "ammo": 12}
      ],
      "decoys": 16
    },
    "us_losangeles": {
      "id": "us_losangeles", "name": "Improved Los Angeles SSN", "kind": "Sub",
      "hp": 159, "max_speed_kts": 32, "cruise_speed_kts": 8, "turn_rate_deg_s": 2.2,
      "rcs": "VerySmall", "noise_db": 26, "visual_factor": 0.15, "vp_value": 30,
      "max_depth_m": 450,
      "sensors": ["sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "esm_std", "visual"],
      "weapons": [
        {"weapon": "torp_homing", "mounts": 4, "ammo": 6},
        {"weapon": "slcm", "mounts": 1, "ammo": 12}
      ],
      "decoys": 16
    },
    "us_ticonderoga": {
      "id": "us_ticonderoga", "name": "Ticonderoga (VLS) CG", "kind": "Ship",
      "hp": 295, "max_speed_kts": 30, "cruise_speed_kts": 20, "turn_rate_deg_s": 1.9,
      "rcs": "Medium", "noise_db": 46, "vp_value": 38,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "sam_long", "mounts": 1, "ammo": 96},
        {"weapon": "sam_medium", "mounts": 1, "ammo": 24},
        {"weapon": "asw_rocket", "mounts": 1, "ammo": 8},
        {"weapon": "slcm", "mounts": 1, "ammo": 26},
        {"weapon": "ascm_std", "mounts": 2, "ammo": 4},
        {"weapon": "gun_5in", "mounts": 1, "ammo": 40},
        {"weapon": "ciws_30mm", "mounts": 2, "ammo": 30},
        {"weapon": "lwt", "mounts": 2, "ammo": 3}
      ],
      "chaff": 60, "decoys": 16
    },
    "us_burke_2a": {
      "id": "us_burke_2a", "name": "Arleigh Burke Flight IIA DDG", "kind": "Ship",
      "hp": 344, "max_speed_kts": 32, "cruise_speed_kts": 20, "turn_rate_deg_s": 2.0,
      "rcs": "Small", "noise_db": 46, "vp_value": 34,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "sam_long", "mounts": 1, "ammo": 72},
        {"weapon": "sam_medium", "mounts": 1, "ammo": 32},
        {"weapon": "asw_rocket", "mounts": 1, "ammo": 4},
        {"weapon": "slcm", "mounts": 1, "ammo": 20},
        {"weapon": "gun_5in", "mounts": 1, "ammo": 40},
        {"weapon": "ciws_30mm", "mounts": 2, "ammo": 30},
        {"weapon": "lwt", "mounts": 2, "ammo": 3}
      ],
      "chaff": 60, "decoys": 16
    },
    "ru_kuznetsov": {
      "id": "ru_kuznetsov", "name": "Pr. 1143.5 Admiral Kuznetsov CV", "kind": "Ship",
      "hp": 1110, "max_speed_kts": 29, "cruise_speed_kts": 18, "turn_rate_deg_s": 1.3,
      "rcs": "Large", "noise_db": 66, "vp_value": 65,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "rasl_km", "mounts": 1, "ammo": 12},
        {"weapon": "sam_point", "mounts": 4, "ammo": 12},
        {"weapon": "ciws_30mm", "mounts": 8, "ammo": 8},
        {"weapon": "gun_30mm", "mounts": 6, "ammo": 10},
        {"weapon": "asw_rocket", "mounts": 2, "ammo": 10}
      ],
      "chaff": 60, "decoys": 16,
      "deck": {"capacity": 20, "cycle_s": 120, "wing": [
        {"class": "fighter", "count": 12},
        {"class": "striker", "count": 4},
        {"class": "helo_asw", "count": 4}
      ]}
    },
    "ru_kilo": {
      "id": "ru_kilo", "name": "Pr. 636.3 Varshavyanka [Improved Kilo] SS", "kind": "Sub",
      "hp": 91, "max_speed_kts": 20, "cruise_speed_kts": 6, "turn_rate_deg_s": 2.5,
      "rcs": "VerySmall", "noise_db": 26, "visual_factor": 0.15, "vp_value": 22,
      "max_depth_m": 200,
      "sensors": ["sonar_p_hull", "sonar_a_hull", "esm_std", "visual"],
      "weapons": [
        {"weapon": "torp_homing", "mounts": 2, "ammo": 8},
        {"weapon": "ascm_std", "mounts": 1, "ammo": 4}
      ],
      "decoys": 8
    },
    "ru_slava": {
      "id": "ru_slava", "name": "Pr. 1164 Atlant [Slava] CG", "kind": "Ship",
      "hp": 284, "max_speed_kts": 32, "cruise_speed_kts": 18, "turn_rate_deg_s": 1.8,
      "rcs": "Medium", "noise_db": 58, "vp_value": 40,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "rasl_km", "mounts": 2, "ammo": 8},
        {"weapon": "sam_long", "mounts": 1, "ammo": 64},
        {"weapon": "sam_point", "mounts": 2, "ammo": 10},
        {"weapon": "gun_130mm", "mounts": 1, "ammo": 40},
        {"weapon": "ciws_30mm", "mounts": 6, "ammo": 10},
        {"weapon": "asw_rocket", "mounts": 2, "ammo": 6},
        {"weapon": "torp_homing", "mounts": 2, "ammo": 5}
      ],
      "chaff": 60, "decoys": 12
    },
    "ru_kirov": {
      "id": "ru_kirov", "name": "Pr. 1144 Kirov CGN", "kind": "Ship",
      "hp": 640, "max_speed_kts": 31, "cruise_speed_kts": 20, "turn_rate_deg_s": 1.8,
      "rcs": "Large", "noise_db": 62, "vp_value": 55,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "rasl_km", "mounts": 1, "ammo": 20},
        {"weapon": "sam_long", "mounts": 1, "ammo": 96},
        {"weapon": "sam_point", "mounts": 2, "ammo": 10},
        {"weapon": "gun_100mm", "mounts": 2, "ammo": 40},
        {"weapon": "ciws_30mm", "mounts": 8, "ammo": 8},
        {"weapon": "asw_rocket", "mounts": 2, "ammo": 6},
        {"weapon": "torp_homing", "mounts": 2, "ammo": 5}
      ],
      "chaff": 60, "decoys": 20
    },
    "ru_sovremenny": {
      "id": "ru_sovremenny", "name": "Pr. 956 Sarych [Sovremenny] DDG", "kind": "Ship",
      "hp": 223, "max_speed_kts": 32, "cruise_speed_kts": 18, "turn_rate_deg_s": 2.0,
      "rcs": "Medium", "noise_db": 58, "vp_value": 26,
      "sensors": ["radar_air_med", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "rasl_km", "mounts": 2, "ammo": 4},
        {"weapon": "sam_medium", "mounts": 2, "ammo": 12},
        {"weapon": "gun_130mm", "mounts": 2, "ammo": 40},
        {"weapon": "ciws_30mm", "mounts": 4, "ammo": 15},
        {"weapon": "asw_rocket", "mounts": 2, "ammo": 6},
        {"weapon": "torp_homing", "mounts": 2, "ammo": 2}
      ],
      "chaff": 40, "decoys": 12
    },
    "ru_udaloy": {
      "id": "ru_udaloy", "name": "Pr. 1155 Udaloy DD", "kind": "Ship",
      "hp": 232, "max_speed_kts": 30, "cruise_speed_kts": 16, "turn_rate_deg_s": 2.0,
      "rcs": "Medium", "noise_db": 44, "vp_value": 28,
      "sensors": ["radar_air_med", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "sam_point", "mounts": 2, "ammo": 16},
        {"weapon": "gun_100mm", "mounts": 2, "ammo": 40},
        {"weapon": "ciws_30mm", "mounts": 4, "ammo": 15},
        {"weapon": "asw_rocket", "mounts": 2, "ammo": 6},
        {"weapon": "torp_homing", "mounts": 2, "ammo": 4}
      ],
      "chaff": 40, "decoys": 12
    },
    "uk_qe": {
      "id": "uk_qe", "name": "Queen Elizabeth (ii) CV", "kind": "Ship",
      "hp": 1052, "max_speed_kts": 27, "cruise_speed_kts": 18, "turn_rate_deg_s": 1.2,
      "rcs": "Large", "noise_db": 64, "vp_value": 60,
      "sensors": ["radar_air_med", "radar_surf_med", "esm_std", "visual"],
      "weapons": [
        {"weapon": "ciws_30mm", "mounts": 3, "ammo": 15},
        {"weapon": "gun_30mm", "mounts": 2, "ammo": 20}
      ],
      "chaff": 40, "decoys": 12,
      "deck": {"capacity": 36, "cycle_s": 120, "wing": [
        {"class": "fighter", "count": 24},
        {"class": "aew", "count": 4},
        {"class": "helo_asw", "count": 8}
      ]}
    },
    "uk_vanguard": {
      "id": "uk_vanguard", "name": "Vanguard SSBN", "kind": "Sub",
      "hp": 271, "max_speed_kts": 25, "cruise_speed_kts": 8, "turn_rate_deg_s": 2.0,
      "rcs": "VerySmall", "noise_db": 24, "visual_factor": 0.12, "vp_value": 45,
      "max_depth_m": 300,
      "sensors": ["sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "esm_std", "visual"],
      "weapons": [
        {"weapon": "torp_homing", "mounts": 2, "ammo": 8}
      ],
      "decoys": 12
    },
    "uk_astute": {
      "id": "uk_astute", "name": "Astute SSN", "kind": "Sub",
      "hp": 162, "max_speed_kts": 29, "cruise_speed_kts": 10, "turn_rate_deg_s": 2.2,
      "rcs": "VerySmall", "noise_db": 22, "visual_factor": 0.12, "vp_value": 32,
      "max_depth_m": 450,
      "sensors": ["sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "esm_std", "visual"],
      "weapons": [
        {"weapon": "torp_homing", "mounts": 3, "ammo": 8},
        {"weapon": "slcm", "mounts": 1, "ammo": 14}
      ],
      "decoys": 22
    },
    "uk_type45": {
      "id": "uk_type45", "name": "Type 45 (Daring) DDG", "kind": "Ship",
      "hp": 246, "max_speed_kts": 31, "cruise_speed_kts": 18, "turn_rate_deg_s": 2.0,
      "rcs": "Small", "noise_db": 44, "vp_value": 30,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "sam_long", "mounts": 1, "ammo": 32},
        {"weapon": "sam_point", "mounts": 1, "ammo": 16},
        {"weapon": "ascm_std", "mounts": 1, "ammo": 4},
        {"weapon": "gun_5in", "mounts": 1, "ammo": 40},
        {"weapon": "gun_30mm", "mounts": 2, "ammo": 20},
        {"weapon": "ciws_30mm", "mounts": 2, "ammo": 15}
      ],
      "chaff": 32, "decoys": 12
    },
    "uk_type23": {
      "id": "uk_type23", "name": "Type 23 (Duke) FF", "kind": "Ship",
      "hp": 196, "max_speed_kts": 30, "cruise_speed_kts": 18, "turn_rate_deg_s": 2.4,
      "rcs": "VerySmall", "noise_db": 34, "vp_value": 20,
      "sensors": ["radar_air_med", "radar_surf_sm", "esm_std", "sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "ascm_std", "mounts": 2, "ammo": 2},
        {"weapon": "sam_point", "mounts": 4, "ammo": 8},
        {"weapon": "gun_5in", "mounts": 1, "ammo": 40},
        {"weapon": "gun_30mm", "mounts": 2, "ammo": 20},
        {"weapon": "lwt", "mounts": 2, "ammo": 12}
      ],
      "chaff": 32, "decoys": 12
    },
    "uk_type26": {
      "id": "uk_type26", "name": "Type 26 (City) FFG", "kind": "Ship",
      "hp": 263, "max_speed_kts": 28, "cruise_speed_kts": 18, "turn_rate_deg_s": 2.3,
      "rcs": "Medium", "noise_db": 44, "vp_value": 28,
      "sensors": ["radar_air_med", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_p_tow", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "sam_point", "mounts": 6, "ammo": 8},
        {"weapon": "asw_rocket", "mounts": 1, "ammo": 8},
        {"weapon": "gun_5in", "mounts": 1, "ammo": 40},
        {"weapon": "gun_30mm", "mounts": 2, "ammo": 20},
        {"weapon": "ciws_30mm", "mounts": 2, "ammo": 15}
      ],
      "chaff": 32, "decoys": 12
    }
  },
  "weapons": {
    "rasl_km": {
      "id": "rasl_km", "name": "Supersonic Anti-Ship Missile",
      "kind": "Ascm",
      "targets": {"surface": true, "land": true},
      "range_nm": 250, "min_range_nm": 5, "speed_kts": 900,
      "pk": 0.50, "salvo": 1, "reload_s": 60, "damage": 45,
      "guidance": "SeaSkimmer"
    },
    "sam_medium": {
      "id": "sam_medium", "name": "Medium-Range SAM",
      "kind": "Sam",
      "targets": {"air": true, "missile": true},
      "range_nm": 30, "min_range_nm": 0.3, "speed_kts": 1800,
      "pk": 0.50, "salvo": 2, "reload_s": 6, "damage": 22,
      "guidance": "HomingAir", "directors": 4
    },
    "gun_130mm": {
      "id": "gun_130mm", "name": "130mm Gun",
      "kind": "Gun",
      "targets": {"surface": true, "land": true},
      "range_nm": 9, "speed_kts": 0, "pk": 0.30, "salvo": 2,
      "reload_s": 10, "damage": 20,
      "guidance": "BallisticGun"
    },
    "gun_100mm": {
      "id": "gun_100mm", "name": "100mm Gun",
      "kind": "Gun",
      "targets": {"surface": true, "land": true},
      "range_nm": 8, "speed_kts": 0, "pk": 0.28, "salvo": 2,
      "reload_s": 8, "damage": 12,
      "guidance": "BallisticGun"
    },
    "gun_76mm": {
      "id": "gun_76mm", "name": "76mm Gun",
      "kind": "Gun",
      "targets": {"surface": true, "air": true, "land": true},
      "range_nm": 6, "speed_kts": 0, "pk": 0.25, "salvo": 3,
      "reload_s": 5, "damage": 8,
      "guidance": "BallisticGun"
    }
  },
  "sensors": {}
}"#;
    serde_json::from_str(FLEET_DB_JSON).expect("fleet database extras must parse")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extra() -> Database {
        db()
    }

    fn built_in() -> Database {
        Database::built_in()
    }

    #[test]
    fn fleet_extras_parse_and_cover_all_classes() {
        let extra = extra();
        assert_eq!(extra.platforms.len(), FLEET_IDS.len());
        for id in FLEET_IDS {
            let p = extra.platform(id).unwrap_or_else(|_| panic!("missing platform {id}"));
            assert_eq!(p.id, id);
            assert!(p.hp > 0.0, "{id} hp");
            assert!(p.max_speed_kts > 0.0, "{id} speed");
            assert!(p.vp_value > 0.0, "{id} vp");
            assert!(!p.sensors.is_empty(), "{id} sensors");
        }
    }

    #[test]
    fn built_in_merges_and_validates_fleet_layer() {
        let db = built_in();
        for id in FLEET_IDS {
            let p = db.platform(id).unwrap_or_else(|_| panic!("merged db missing {id}"));
            for s in &p.sensors {
                db.sensor(s).unwrap_or_else(|_| panic!("{id} sensor {s} missing"));
            }
            for w in &p.weapons {
                db.weapon(&w.weapon)
                    .unwrap_or_else(|_| panic!("{id} weapon {} missing", w.weapon));
            }
        }
        db.validate().expect("built-in + fleet layer must validate");
    }

    #[test]
    fn every_class_keeps_its_volume_damage_point_total() {
        let db = built_in();
        for (id, dp) in KNOWN_DP {
            let p = db.platform(id).unwrap_or_else(|_| panic!("missing {id}"));
            assert_eq!(p.hp, dp, "{id} DP drifted from the volume total");
        }
    }

    #[test]
    fn spot_check_speed_hp_and_signature_magnitudes() {
        let db = built_in();
        // China's Navy A-9: 0 51 102 153 184 204; Surf Speed 29.
        let d = db.platform("cn_052d").unwrap();
        assert_eq!(d.hp, 204.0);
        assert_eq!(d.max_speed_kts, 29.0);
        // America's Navy A-31: 0 86 172 258 310 344; Surf Speed 32.
        let b = db.platform("us_burke_2a").unwrap();
        assert_eq!(b.hp, 344.0);
        assert_eq!(b.max_speed_kts, 32.0);
        // Commonwealth Navies A-33: 0 41 81 122 146 162; Subm Speed 29.
        let a = db.platform("uk_astute").unwrap();
        assert_eq!(a.hp, 162.0);
        assert_eq!(a.max_speed_kts, 29.0);
        // Carriers and conventional submarines must stay in their lanes.
        assert!(db.platform("us_nimitz").unwrap().hp > 1_000.0);
        assert!(db.platform("cn_002").unwrap().hp > 1_000.0);
        assert!(db.platform("ru_kilo").unwrap().hp < 100.0);
        assert!(db.platform("ru_kilo").unwrap().max_speed_kts <= 20.0);
    }

    #[test]
    fn new_weapons_do_not_shadow_builtin_ids() {
        let base = serde_json::from_str::<Database>(crate::db_data::BUILT_IN)
            .expect("built-in JSON");
        let extra = extra();
        for id in ["rasl_km", "sam_medium", "gun_130mm", "gun_100mm", "gun_76mm"] {
            let w = extra.weapon(id).unwrap_or_else(|_| panic!("missing new weapon {id}"));
            assert!(!base.weapons.contains_key(id), "{id} would shadow built-in");
            assert!(w.range_nm > 0.0);
            assert!(w.pk > 0.0 && w.pk <= 1.0);
            assert!(w.damage > 0.0);
        }
        let s = extra.weapon("rasl_km").unwrap();
        assert_eq!(s.kind, crate::db::WeaponKind::Ascm);
        assert_eq!(s.speed_kts, 900.0);
        assert_eq!(s.pk, 0.5);
        assert_eq!(s.damage, 45.0);
    }

    #[test]
    fn every_new_weapon_is_referenced_by_a_fleet_platform() {
        let extra = extra();
        let mut used = std::collections::BTreeSet::new();
        for p in extra.platforms.values() {
            for w in &p.weapons {
                used.insert(w.weapon.as_str());
            }
        }
        for id in ["rasl_km", "sam_medium", "gun_130mm", "gun_100mm", "gun_76mm"] {
            assert!(used.contains(id), "new weapon {id} is not fitted to any class");
        }
        // The generic supersonic ASM is the Russian heavy ASCM fit.
        let slava = extra.platform("ru_slava").unwrap();
        assert!(slava.weapons.iter().any(|w| w.weapon == "rasl_km"));
    }

    #[test]
    fn carrier_decks_resolve_to_aircraft_classes() {
        let db = built_in();
        for id in ["cn_002", "us_nimitz", "us_ford", "ru_kuznetsov", "uk_qe"] {
            let p = db.platform(id).unwrap();
            let deck = p.deck.as_ref().unwrap_or_else(|| panic!("{id} has no deck"));
            assert!(deck.capacity > 0);
            let total: u16 = deck.wing.iter().map(|w| w.count as u16).sum();
            assert!(total > 0, "{id} empty wing");
            for w in &deck.wing {
                assert_eq!(db.platform(&w.class).unwrap().kind, crate::types::PlatformKind::Aircraft);
            }
        }
    }

    #[test]
    fn submarine_depth_and_signature_are_plausible() {
        let db = built_in();
        for (id, min_depth) in [
            ("cn_039a", 250.0),
            ("cn_kilo", 200.0),
            ("ru_kilo", 200.0),
            ("us_losangeles", 300.0),
            ("us_virginia", 300.0),
            ("uk_astute", 300.0),
            ("uk_vanguard", 250.0),
        ] {
            let p = db.platform(id).unwrap();
            assert_eq!(p.kind, crate::types::PlatformKind::Sub, "{id}");
            assert!(p.max_depth_m >= min_depth, "{id} depth {}", p.max_depth_m);
            assert!(p.noise_db <= 34.0, "{id} should be a quiet submarine");
            assert!(p.visual_factor < 0.5, "{id} visual factor");
        }
    }
}

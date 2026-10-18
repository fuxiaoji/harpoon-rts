//! Canonical built-in database (flavored generic 1990s–2020s order of battle).
//!
//! Calibration notes (see docs/design/adaptations.md for the full record):
//! - hp pools follow Admiralty Standard Damage Point magnitudes
//!   (CVN ≈ 1300 DP, large DDG ≈ 200–300, FFG ≈ 175).
//! - Gun/missile damage follows the same tables (5in ≈ 15–20, heavy ASCM
//!   ≈ 40+, heavyweight torpedo ≈ 200+).
//! - Jet max speeds are afterburner values; cruise speeds are military power.
//! - Sonar/ESM ranges sit inside the rule-table ceilings (passive hull ≤ 8,
//!   towed ≤ 10, ESM ≈ visual × generation multiplier).

pub const BUILT_IN: &str = r#"{
  "platforms": {
    "cvn": {
      "id": "cvn", "name": "Fleet Carrier", "kind": "Ship",
      "hp": 1300, "max_speed_kts": 32, "cruise_speed_kts": 25, "turn_rate_deg_s": 1.2,
      "rcs": "Large", "noise_db": 66, "vp_value": 60,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "visual"],
      "weapons": [
        {"weapon": "ciws_30mm", "mounts": 2, "ammo": 60},
        {"weapon": "sam_point", "mounts": 2, "ammo": 24},
        {"weapon": "gun_5in", "mounts": 1, "ammo": 40}
      ],
      "chaff": 60, "decoys": 14,
      "deck": {"capacity": 20, "cycle_s": 120, "wing": [
        {"class": "fighter", "count": 8},
        {"class": "striker", "count": 4},
        {"class": "helo_asw", "count": 4}
      ]}
    },
    "ddg_aegis": {
      "id": "ddg_aegis", "name": "Aegis Destroyer", "kind": "Ship",
      "hp": 260, "max_speed_kts": 30, "cruise_speed_kts": 20, "turn_rate_deg_s": 1.8,
      "rcs": "Medium", "noise_db": 60, "vp_value": 30,
      "sensors": ["radar_air_long", "radar_surf_med", "esm_std", "sonar_p_hull", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "sam_long", "mounts": 1, "ammo": 48},
        {"weapon": "sam_point", "mounts": 1, "ammo": 24},
        {"weapon": "ciws_30mm", "mounts": 1, "ammo": 60},
        {"weapon": "gun_5in", "mounts": 1, "ammo": 40},
        {"weapon": "slcm", "mounts": 2, "ammo": 1}
      ],
      "chaff": 50, "decoys": 12
    },
    "ffg": {
      "id": "ffg", "name": "Frigate", "kind": "Ship",
      "hp": 175, "max_speed_kts": 28, "cruise_speed_kts": 18, "turn_rate_deg_s": 2.2,
      "rcs": "Medium", "noise_db": 56, "vp_value": 18,
      "sensors": ["radar_air_med", "radar_surf_sm", "esm_std", "sonar_p_tow", "sonar_a_hull", "visual"],
      "weapons": [
        {"weapon": "sam_point", "mounts": 1, "ammo": 24},
        {"weapon": "ciws_30mm", "mounts": 1, "ammo": 40},
        {"weapon": "gun_5in", "mounts": 1, "ammo": 40},
        {"weapon": "asw_rocket", "mounts": 1, "ammo": 12}
      ],
      "chaff": 30, "decoys": 8
    },
    "mgb": {
      "id": "mgb", "name": "Missile Boat", "kind": "Ship",
      "hp": 45, "max_speed_kts": 36, "cruise_speed_kts": 18, "turn_rate_deg_s": 4.0,
      "rcs": "Small", "noise_db": 52, "vp_value": 8,
      "sensors": ["radar_surf_sm", "visual"],
      "weapons": [
        {"weapon": "ascm_std", "mounts": 4, "ammo": 2},
        {"weapon": "gun_30mm", "mounts": 1, "ammo": 40}
      ],
      "chaff": 10, "decoys": 2
    },
    "pb": {
      "id": "pb", "name": "Patrol Boat", "kind": "Ship",
      "hp": 25, "max_speed_kts": 32, "cruise_speed_kts": 20, "turn_rate_deg_s": 5.0,
      "rcs": "VerySmall", "noise_db": 48, "vp_value": 4,
      "sensors": ["radar_nav", "visual"],
      "weapons": [{"weapon": "gun_30mm", "mounts": 1, "ammo": 40}],
      "chaff": 4
    },
    "ssk": {
      "id": "ssk", "name": "Diesel Submarine", "kind": "Sub",
      "hp": 60, "max_speed_kts": 20, "cruise_speed_kts": 6, "turn_rate_deg_s": 3.0,
      "rcs": "VerySmall", "noise_db": 24, "visual_factor": 0.15, "vp_value": 25,
      "max_depth_m": 250,
      "sensors": ["sonar_p_hull", "sonar_a_hull", "visual"],
      "weapons": [{"weapon": "torp_homing", "mounts": 6, "ammo": 18}],
      "decoys": 10
    },
    "fighter": {
      "id": "fighter", "name": "Multifighter", "kind": "Aircraft",
      "hp": 30, "max_speed_kts": 1100, "cruise_speed_kts": 450, "turn_rate_deg_s": 12.0,
      "rcs": "Small", "vp_value": 12, "endurance_s": 5400, "max_depth_m": 0,
      "sensors": ["radar_air_med", "visual"],
      "weapons": [
        {"weapon": "aam_ir", "mounts": 3, "ammo": 2},
        {"weapon": "asm_air", "mounts": 2, "ammo": 1}
      ],
      "chaff": 12
    },
    "striker": {
      "id": "striker", "name": "Strike Fighter", "kind": "Aircraft",
      "hp": 35, "max_speed_kts": 950, "cruise_speed_kts": 420, "turn_rate_deg_s": 10.0,
      "rcs": "Small", "vp_value": 14, "endurance_s": 7200,
      "sensors": ["radar_surf_sm", "visual"],
      "weapons": [
        {"weapon": "asm_air", "mounts": 4, "ammo": 1},
        {"weapon": "aam_ir", "mounts": 1, "ammo": 2}
      ],
      "chaff": 12
    },
    "bomber": {
      "id": "bomber", "name": "Strike Bomber", "kind": "Aircraft",
      "hp": 45, "max_speed_kts": 650, "cruise_speed_kts": 400, "turn_rate_deg_s": 6.0,
      "rcs": "Large", "vp_value": 16, "endurance_s": 9000,
      "sensors": ["radar_surf_sm", "visual"],
      "weapons": [{"weapon": "asm_air", "mounts": 4, "ammo": 1}],
      "chaff": 16
    },
    "aew": {
      "id": "aew", "name": "AEW Aircraft", "kind": "Aircraft",
      "hp": 30, "max_speed_kts": 550, "cruise_speed_kts": 340, "turn_rate_deg_s": 5.0,
      "rcs": "Large", "vp_value": 15, "endurance_s": 14400,
      "sensors": ["radar_air_xlong", "radar_surf_med", "esm_std"],
      "weapons": []
    },
    "helo_asw": {
      "id": "helo_asw", "name": "ASW Helicopter", "kind": "Aircraft",
      "hp": 25, "max_speed_kts": 150, "cruise_speed_kts": 110, "turn_rate_deg_s": 25.0,
      "rcs": "Small", "vp_value": 8, "endurance_s": 10800, "dipping_sonar": true,
      "sensors": ["radar_nav", "sonar_dipping", "visual"],
      "weapons": [{"weapon": "lwt", "mounts": 1, "ammo": 2}]
    },
    "airfield": {
      "id": "airfield", "name": "Coastal Airfield", "kind": "Base",
      "hp": 400, "max_speed_kts": 0, "turn_rate_deg_s": 0,
      "rcs": "Large", "vp_value": 40, "is_base": true,
      "sensors": ["radar_air_long", "esm_std"],
      "weapons": [
        {"weapon": "sam_point", "mounts": 2, "ammo": 24},
        {"weapon": "gun_5in", "mounts": 1, "ammo": 40}
      ],
      "chaff": 0,
      "deck": {"capacity": 12, "cycle_s": 180, "wing": [
        {"class": "fighter", "count": 4},
        {"class": "bomber", "count": 4}
      ]}
    },
    "navsta": {
      "id": "navsta", "name": "Naval Station", "kind": "Base",
      "hp": 500, "max_speed_kts": 0, "turn_rate_deg_s": 0,
      "rcs": "Large", "vp_value": 30, "is_base": true,
      "sensors": ["radar_surf_med", "esm_std"],
      "weapons": [
        {"weapon": "sam_point", "mounts": 1, "ammo": 16},
        {"weapon": "gun_5in", "mounts": 1, "ammo": 40}
      ],
      "chaff": 0
    }
  },
  "weapons": {
    "gun_5in": {
      "id": "gun_5in", "name": "5in Gun", "kind": "Gun",
      "targets": {"surface": true, "land": true},
      "range_nm": 11, "speed_kts": 0, "pk": 0.30, "salvo": 2, "reload_s": 10, "damage": 16,
      "guidance": "BallisticGun"
    },
    "gun_30mm": {
      "id": "gun_30mm", "name": "30mm Gun", "kind": "Gun",
      "targets": {"surface": true, "air": true},
      "range_nm": 3, "speed_kts": 0, "pk": 0.15, "salvo": 3, "reload_s": 4, "damage": 8,
      "guidance": "BallisticGun"
    },
    "ciws_30mm": {
      "id": "ciws_30mm", "name": "CIWS", "kind": "Ciws",
      "targets": {"missile": true, "air": true, "surface": true},
      "range_nm": 1.6, "speed_kts": 0, "pk": 0.30, "salvo": 3, "reload_s": 2.5, "damage": 15,
      "guidance": "BallisticGun"
    },
    "sam_long": {
      "id": "sam_long", "name": "Long-Range SAM", "kind": "Sam",
      "targets": {"air": true, "missile": true},
      "range_nm": 70, "min_range_nm": 2, "speed_kts": 2000, "pk": 0.55, "salvo": 2,
      "reload_s": 7, "damage": 30, "guidance": "HomingAir"
    },
    "sam_point": {
      "id": "sam_point", "name": "Point-Defense SAM", "kind": "Sam",
      "targets": {"air": true, "missile": true},
      "range_nm": 14, "min_range_nm": 0.3, "speed_kts": 1500, "pk": 0.42, "salvo": 1,
      "reload_s": 4, "damage": 25, "guidance": "HomingAir"
    },
    "aam_ir": {
      "id": "aam_ir", "name": "IR AAM", "kind": "Aam", "ir": true,
      "targets": {"air": true},
      "range_nm": 15, "min_range_nm": 0.3, "speed_kts": 2000, "pk": 0.50, "salvo": 1,
      "reload_s": 3, "damage": 20, "guidance": "HomingAir"
    },
    "ascm_std": {
      "id": "ascm_std", "name": "Anti-Ship Cruise Missile", "kind": "Ascm",
      "targets": {"surface": true, "land": true},
      "range_nm": 60, "min_range_nm": 3, "speed_kts": 560, "pk": 0.55, "salvo": 2,
      "reload_s": 45, "damage": 42, "guidance": "SeaSkimmer"
    },
    "asm_air": {
      "id": "asm_air", "name": "Air-Launched ASM", "kind": "Ascm",
      "targets": {"surface": true, "land": true},
      "range_nm": 45, "min_range_nm": 5, "speed_kts": 600, "pk": 0.50, "salvo": 1,
      "reload_s": 30, "damage": 40, "guidance": "SeaSkimmer"
    },
    "slcm": {
      "id": "slcm", "name": "Land-Attack Cruise Missile", "kind": "Slcm",
      "targets": {"land": true},
      "range_nm": 160, "min_range_nm": 10, "speed_kts": 480, "pk": 0.75, "salvo": 1,
      "reload_s": 60, "damage": 60, "guidance": "SeaSkimmer"
    },
    "torp_homing": {
      "id": "torp_homing", "name": "Active-Homing Torpedo", "kind": "Torpedo",
      "targets": {"surface": true, "sub": true},
      "range_nm": 14, "speed_kts": 42, "enable_range_nm": 2.5, "pk": 0.70, "salvo": 1,
      "reload_s": 30, "damage": 250, "guidance": "TorpedoActive"
    },
    "torp_wake": {
      "id": "torp_wake", "name": "Wake-Homing Torpedo", "kind": "Torpedo",
      "targets": {"surface": true},
      "range_nm": 20, "speed_kts": 48, "enable_range_nm": 0.5, "pk": 0.68, "salvo": 1,
      "reload_s": 30, "damage": 280, "guidance": "WakeHome"
    },
    "lwt": {
      "id": "lwt", "name": "Lightweight ASW Torpedo", "kind": "Lwt",
      "targets": {"sub": true},
      "range_nm": 6, "speed_kts": 45, "enable_range_nm": 0.4, "pk": 0.55, "salvo": 1,
      "reload_s": 20, "damage": 45, "guidance": "TorpedoActive"
    },
    "asw_rocket": {
      "id": "asw_rocket", "name": "ASW Rocket", "kind": "AswRocket",
      "targets": {"sub": true},
      "range_nm": 6, "speed_kts": 0, "pk": 0.13, "salvo": 2, "reload_s": 15, "damage": 25,
      "guidance": "AswBallistic"
    }
  },
  "sensors": {
    "radar_air_xlong": {
      "id": "radar_air_xlong", "name": "AEW Radar", "kind": "Radar", "alt_m": 8500,
      "air_range_nm": 260, "surface_range_nm": 60, "missile_range_nm": 200
    },
    "radar_air_long": {
      "id": "radar_air_long", "name": "Long-Range Air Radar", "kind": "Radar", "alt_m": 30,
      "air_range_nm": 200, "surface_range_nm": 45, "missile_range_nm": 100
    },
    "radar_air_med": {
      "id": "radar_air_med", "name": "Medium Air Radar", "kind": "Radar", "alt_m": 25,
      "air_range_nm": 110, "surface_range_nm": 28, "missile_range_nm": 50
    },
    "radar_surf_med": {
      "id": "radar_surf_med", "name": "Medium Surface Radar", "kind": "Radar", "alt_m": 22,
      "air_range_nm": 60, "surface_range_nm": 24, "missile_range_nm": 35
    },
    "radar_surf_sm": {
      "id": "radar_surf_sm", "name": "Surface Search Radar", "kind": "Radar", "alt_m": 18,
      "air_range_nm": 45, "surface_range_nm": 18, "missile_range_nm": 25
    },
    "radar_nav": {
      "id": "radar_nav", "name": "Navigation Radar", "kind": "Radar", "alt_m": 10,
      "air_range_nm": 25, "surface_range_nm": 12, "missile_range_nm": 12
    },
    "esm_std": {
      "id": "esm_std", "name": "ESM Intercept", "kind": "Esm", "alt_m": 20,
      "air_range_nm": 120, "surface_range_nm": 20, "missile_range_nm": 25
    },
    "visual": {
      "id": "visual", "name": "Visual Lookouts", "kind": "Visual", "alt_m": 25,
      "air_range_nm": 18, "surface_range_nm": 11, "missile_range_nm": 5
    },
    "sonar_p_hull": {
      "id": "sonar_p_hull", "name": "Passive Sonar", "kind": "SonarPassive", "alt_m": 0,
      "sub_range_nm": 8
    },
    "sonar_p_tow": {
      "id": "sonar_p_tow", "name": "Towed Passive Array", "kind": "SonarPassive", "alt_m": 0,
      "sub_range_nm": 10
    },
    "sonar_a_hull": {
      "id": "sonar_a_hull", "name": "Active Sonar", "kind": "SonarActive", "alt_m": 0,
      "sub_range_nm": 6
    },
    "sonar_dipping": {
      "id": "sonar_dipping", "name": "Dipping Sonar", "kind": "SonarActive", "alt_m": 0,
      "sub_range_nm": 4
    }
  }
}"#;

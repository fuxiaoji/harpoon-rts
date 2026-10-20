//! TacticalStaff vs TacticalStaff on dual_carrier_kills — full AAR.
//! Run: cargo run --release -p harpoon-server --example duel_carrier_kills


use harpoon_core::db::Database;
use harpoon_core::scenario::{load_scenario, Scenario};
use harpoon_core::types::Side;
use harpoon_core::EventKind;
use std::collections::BTreeMap;

/// 简化版状态机参谋——与 TacticalStaff 同构但内联到 example 中。
struct SimpleTactical { side: Side }
impl SimpleTactical {
    fn new(side: Side) -> Self { Self { side } }
    fn decide(&mut self, obs: &harpoon_core::observe::Observation) -> Vec<harpoon_core::Command> {
        use harpoon_core::commands::{AirMission, Command};
        use harpoon_core::types::PlatformKind;
        let mut cmds = Vec::new();
        let enemy = self.side.other();
        for u in &obs.units {
            if !u.fire_on && u.kind == PlatformKind::Ship {
                cmds.push(Command::SetWeaponsFree { unit: u.id, free: true });
            }
            match u.kind {
                PlatformKind::Sub => {
                    for c in &obs.contacts {
                        if c.category == PlatformKind::Ship && !c.stale && c.quality >= 0.4 {
                            cmds.push(Command::AssignTarget { unit: u.id, contact: c.id });
                            let d = ((u.pos.x - c.estimate.x).powi(2) + (u.pos.y - c.estimate.y).powi(2)).sqrt();
                            if d < 9.0 {
                                cmds.push(Command::Fire { unit: u.id, contact: c.id, weapon: "torp_homing".into(), rounds: 2 });
                                cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 18.0 });
                                cmds.push(Command::SetCourse { unit: u.id, course_deg: (u.course_deg + 120.0) % 360.0 });
                            } else if d > 5.0 {
                                cmds.push(Command::SetCourse { unit: u.id, course_deg: u.pos.bearing_to(c.estimate) });
                                cmds.push(Command::SetSpeed { unit: u.id, speed_kts: 4.0 });
                            }
                        }
                    }
                }
                PlatformKind::Ship => {
                    // 防空：武器自由下自动交战
                    // 对海：指派最近接触
                    let nearest = obs.contacts.iter()
                        .filter(|c| c.category == PlatformKind::Ship && !c.stale && c.quality > 0.3)
                        .min_by(|a, b| {
                            let da = (a.estimate.x - u.pos.x).powi(2) + (a.estimate.y - u.pos.y).powi(2);
                            let db = (b.estimate.x - u.pos.x).powi(2) + (b.estimate.y - u.pos.y).powi(2);
                            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                        });
                    if let Some(c) = nearest {
                        cmds.push(Command::AssignTarget { unit: u.id, contact: c.id });
                    }
                    // 对地：SLCM 打基地
                    let base = obs.contacts.iter()
                        .find(|c| c.category == PlatformKind::Base && !c.stale && c.quality >= 0.25);
                    if let Some(bc) = base {
                        let has_slcm = u.weapons.iter().any(|w| w.weapon == "slcm" && w.ammo > 0);
                        if has_slcm {
                            cmds.push(Command::Fire { unit: u.id, contact: bc.id, weapon: "slcm".into(), rounds: 1 });
                        }
                    }
                }
                PlatformKind::Aircraft => {
                    // CAP: 指派最近的空中接触
                    let air = obs.contacts.iter()
                        .find(|c| c.category == PlatformKind::Aircraft && !c.stale && c.quality > 0.4);
                    if let Some(c) = air {
                        cmds.push(Command::AssignTarget { unit: u.id, contact: c.id });
                    }
                }
                _ => {}
            }
        }
        // CAP 轮换
        let cap_count = obs.units.iter().filter(|u| u.kind == PlatformKind::Aircraft && u.mission.as_deref() == Some("cap")).count();
        if cap_count < 2 {
            for carrier in obs.units.iter().filter(|u| u.deck.is_some()) {
                if let Some(deck) = &carrier.deck {
                    for (cls, n) in &deck.ready {
                        if *n > 0 && (cls.contains("fighter") || cls.contains("f18") || cls.contains("f35") || cls.contains("j15")) {
                            cmds.push(Command::LaunchAircraft {
                                unit: carrier.id, class: cls.clone(), count: 2,
                                mission: AirMission::Cap {
                                    station: carrier.pos.advanced(90.0, 30.0), radius_nm: 15.0,
                                    follow: true,
                                },
                            });
                            break;
                        }
                    }
                }
            }
        }
        // 打击波次
        let contact_ok = obs.contacts.iter().any(|c| c.category == PlatformKind::Ship && !c.stale && c.quality >= 0.4);
        if contact_ok {
            for carrier in obs.units.iter().filter(|u| u.deck.is_some()) {
                if let Some(deck) = &carrier.deck {
                    for (cls, n) in &deck.ready {
                        if *n > 0 && (cls.contains("striker") || cls.contains("bomber") || cls.contains("j15") || cls.contains("f18")) {
                            if let Some(c) = obs.contacts.iter().find(|c| c.category == PlatformKind::Ship && !c.stale && c.quality >= 0.4) {
                                cmds.push(Command::LaunchAircraft {
                                    unit: carrier.id, class: cls.clone(), count: 2,
                                    mission: AirMission::Strike { contact: c.id },
                                });
                            }
                            break;
                        }
                    }
                }
            }
        }
        let _ = enemy;
        cmds
    }
}

fn main() {
    let path = "scenarios/dual_carrier_kills.json";
    let txt = std::fs::read_to_string(path).expect("scenario");
    let sc: Scenario = serde_json::from_str(&txt).expect("parse");

    // 加载数据卷（与服务器一致），否则 cn/us 军舰类不存在
    let mut db = Database::built_in();
    let vol_dir = std::path::Path::new("database/volumes");
    if let Ok(entries) = std::fs::read_dir(vol_dir) {
        let mut files: Vec<_> = entries.flatten().map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
            .filter(|p| !p.file_name().map(|n| n.to_string_lossy().starts_with('_')).unwrap_or(false))
            .collect();
        files.sort();
        for f in files {
            if let Ok(txt) = std::fs::read_to_string(&f) {
                if let Ok(vol) = Database::from_json(&txt) { db = db.merged(&vol); }
            }
        }
    }
    let mut w = load_scenario(sc.clone(), db).unwrap();
    let mut blue = SimpleTactical::new(Side::Blue);
    let mut red = SimpleTactical::new(Side::Red);

    // Kill/sink/hit tracking per side
    let mut blue_kills = Vec::new();
    let mut red_kills = Vec::new();
    let mut blue_dmg_dealt = 0.0f64;
    let mut red_dmg_dealt = 0.0f64;
    let mut salvos = 0u32;
    let mut sam_shots = 0u32;
    let mut intercepts = 0u32;
    let mut impacts = 0u32;
    let mut gun_actions = 0u32;
    let mut torp_detections = 0u32;
    let mut air_launches = 0u32;
    let mut last_seq = 0u64;

    // Names for reporting
    let unit_names: std::collections::HashMap<u32, String> = w
        .entities
        .values()
        .map(|e| (e.id.0, e.name.clone()))
        .collect();

    let max_ticks = sc.duration_s;
    while !w.over && w.tick < max_ticks {
        // Decision every 10 ticks
        let cmds = if w.tick % 10 == 0 {
            let mut all = Vec::new();
            for side in [Side::Blue, Side::Red] {
                let obs = harpoon_core::observe::observe(&w, side, 0, 50);
                let cmds = match side {
                    Side::Blue => blue.decide(&obs),
                    Side::Red => red.decide(&obs),
                };
                all.extend(cmds.into_iter().map(|c| (side, c)));
            }
            all
        } else {
            Vec::new()
        };

        harpoon_core::sim::tick(&mut w, &cmds);

        for ev in w.events.iter().filter(|e| e.seq > last_seq) {
            match &ev.kind {
                EventKind::WeaponsRelease { side, weapon, .. } => {
                    salvos += 1;
                    if weapon.contains("SAM") || weapon.contains("SAM") { sam_shots += 1; }
                }
                EventKind::SamShot { .. } => { sam_shots += 1; }
                EventKind::Intercept { side, intercepted: true, .. } => { intercepts += 1; }
                EventKind::Impact { side, unit, damage, .. } => {
                    impacts += 1;
                    if *side == Side::Red { blue_dmg_dealt += damage; }
                    else { red_dmg_dealt += damage; }
                    let _ = unit;
                }
                EventKind::GunAction { side, hits, .. } => {
                    gun_actions += 1;
                    if *hits > 0 {
                        if *side == Side::Red { blue_dmg_dealt += 16.0; }
                        else { red_dmg_dealt += 16.0; }
                    }
                }
                EventKind::TorpedoDetected { .. } => { torp_detections += 1; }
                EventKind::AirLaunch { .. } => { air_launches += 1; }
                EventKind::Sunk { side, unit_name, .. } => {
                    let entry = format!("t={} {}", w.tick, unit_name);
                    if *side == Side::Blue {
                        blue_kills.push(format!("Blue lost: {entry}"));
                    } else {
                        red_kills.push(format!("Red lost: {entry}"));
                    }
                }
                EventKind::AirShotDown { side, unit_name, .. } => {
                    let entry = format!("t={} {}", w.tick, unit_name);
                    if *side == Side::Blue {
                        blue_kills.push(format!("Blue lost: {entry}"));
                    } else {
                        red_kills.push(format!("Red lost: {entry}"));
                    }
                }
                _ => {}
            }
        }
        if let Some(e) = w.events.last() { last_seq = e.seq; }
    }

    // Report
    let alive_blue = w.entities.values().filter(|e| e.alive && e.side == Side::Blue && e.kind != PlatformKind::Aircraft).count();
    let alive_red = w.entities.values().filter(|e| e.alive && e.side == Side::Red && e.kind != PlatformKind::Aircraft).count();
    let (vp_b, vp_r) = w.victory_points();

    println!("=== Dual Carrier Kills — TacticalStaff vs TacticalStaff ===");
    println!("duration: {}s / {}", w.tick, max_ticks);
    println!("over: {}  result: {:?}", w.over, w.result.as_ref().map(|r| (&r.winner, &r.reason)));
    println!();
    println!("--- damage ---");
    println!("Blue dealt: {:.0} HP damage, Red dealt: {:.0} HP damage", blue_dmg_dealt, red_dmg_dealt);
    println!();
    println!("--- combat actions ---");
    println!("weapon salvos: {}  SAM shots: {}  intercepts: {}  impacts: {}", salvos, sam_shots, intercepts, impacts);
    println!("gun actions: {}  torpedo detections: {}  air launches: {}", gun_actions, torp_detections, air_launches);
    println!();
    println!("--- ships lost ---");
    for k in &blue_kills { println!("  {}", k); }
    for k in &red_kills { println!("  {}", k); }
    println!();
    println!("--- survivors ---");
    println!("Blue ships/subs alive: {}  Red ships/subs alive: {}", alive_blue, alive_red);
    println!("VP: Blue {:.0} / Red {:.0}", vp_b, vp_r);
    println!();
    println!("--- force status ---");
    for e in w.entities.values().filter(|e| e.alive && !e.kind.is_projectile() && e.kind != PlatformKind::Aircraft) {
        let hp_pct = (e.hp / e.hp_max * 100.0) as i32;
        println!("  {} [{:?}] {} HP {}/{} ({}%)", e.name, e.side, e.class, e.hp as i32, e.hp_max as i32, hp_pct);
    }

    println!("\n--- kill score ---");
    let kill_score = |s: Side| -> f64 {
        w.entities.values()
            .filter(|e| !e.alive && e.side != s && e.kind != PlatformKind::Missile && e.kind != PlatformKind::Torpedo)
            .filter_map(|e| w.db.platforms.get(&e.class))
            .map(|p| p.vp_value)
            .sum()
    };
    println!("Blue kill score: {:.0}  Red kill score: {:.0}", kill_score(Side::Blue), kill_score(Side::Red));
}

// Avoid unused import
use harpoon_core::types::PlatformKind as _PK;
use harpoon_core::types::PlatformKind as PlatformKind;

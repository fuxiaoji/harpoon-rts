//! Referee-only retrospective metrics; never supplied to a controller.
use harpoon_core::{
    observe::observe, types::PlatformKind, EntityId, Event, EventKind, Side, World,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Serialize)]
pub struct Flight {
    unit: EntityId,
    side: Side,
    class: String,
    mission: String,
    born: i64,
    home: Option<EntityId>,
    fate: String,
    ended_at: Option<i64>,
    actual_asm_rounds: u32,
    tracked_seconds: u32,
    fresh_track_seconds: u32,
    longest_fresh_run: u32,
    release_envelope_seconds: u32,
    blocked_seconds: BTreeMap<String, u32>,
    #[serde(skip)]
    run: u32,
    #[serde(skip)]
    last_pos: harpoon_core::Vec2,
    #[serde(skip)]
    rtb: bool,
}
#[derive(Default, Serialize)]
pub struct Audit {
    pub flights: BTreeMap<EntityId, Flight>,
    pub ambiguous_recovery_receipts: u32,
    pub strike_trace: Vec<Value>,
    pub ew_checks: u64,
    pub effective_ew_checks: u64,
    pub ew_trace: Vec<Value>,
}
impl Audit {
    pub fn sample(&mut self, w: &World, events: &[Event], memory: &[Value; 2]) {
        for side in [Side::Blue, Side::Red] {
            let o = observe(w, side, w.event_seq, 0);
            let mem = &memory[side.index()];
            for u in o.units.iter().filter(|u| u.kind == PlatformKind::Aircraft) {
                let m = &mem["air_minds"][u.id.0.to_string()];
                let f = self.flights.entry(u.id).or_insert_with(|| Flight {
                    unit: u.id,
                    side,
                    class: u.class.clone(),
                    mission: u.mission.clone().unwrap_or("unknown".into()),
                    born: w.tick,
                    home: None,
                    fate: "airborne".into(),
                    ended_at: None,
                    actual_asm_rounds: 0,
                    tracked_seconds: 0,
                    fresh_track_seconds: 0,
                    longest_fresh_run: 0,
                    release_envelope_seconds: 0,
                    blocked_seconds: BTreeMap::new(),
                    run: 0,
                    last_pos: u.pos,
                    rtb: false,
                });
                f.home = m["home"].as_u64().map(|id| EntityId(id as u32)).or(f.home);
                f.last_pos = u.pos;
                f.rtb = u.mission.as_deref() == Some("rtb");
                if f.mission != "strike" || f.rtb {
                    continue;
                }
                let c = m["target"]
                    .as_u64()
                    .and_then(|id| o.contacts.iter().find(|c| c.id.0 as u64 == id));
                let range = u
                    .weapons
                    .iter()
                    .filter(|m| m.ammo > 0)
                    .filter_map(|m| w.db.weapons.get(&m.weapon))
                    .filter(|d| d.kind == harpoon_core::db::WeaponKind::Ascm)
                    .map(|d| d.range_nm)
                    .fold(0.0, f64::max);
                let distance = c.map(|c| u.pos.range_to(c.estimate));
                let fresh = c.is_some_and(|c| !c.stale && !c.bearing_only && c.quality >= 0.4);
                f.tracked_seconds += 1;
                if fresh {
                    f.fresh_track_seconds += 1;
                    f.run += 1;
                    f.longest_fresh_run = f.longest_fresh_run.max(f.run);
                } else {
                    f.run = 0;
                }
                let block = if range == 0.0 {
                    "empty"
                } else if !fresh {
                    "track"
                } else if distance.is_none_or(|d| d > range * 0.95) {
                    "distance"
                } else if u.altitude_m > 60.0 {
                    "altitude"
                } else {
                    "window_or_group"
                };
                *f.blocked_seconds.entry(block.into()).or_default() += 1;
                if fresh
                    && distance.is_some_and(|d| d <= range * 0.95)
                    && u.altitude_m <= 60.0
                    && range > 0.0
                {
                    f.release_envelope_seconds += 1;
                }
                self.strike_trace.push(json!({"tick":w.tick,"side":side,"unit":u.id,"target":m["target"],"phase":m["phase"],"altitude_m":u.altitude_m,"fresh":fresh,"distance_nm":distance,"loaded_range_nm":range,"block":block}));
            }
        }
        let mut recoveries: BTreeMap<(usize, String, EntityId), u32> = BTreeMap::new();
        let mut explicit = BTreeSet::new();
        for e in events {
            match &e.kind {
                EventKind::WeaponsRelease {
                    shooter,
                    weapon,
                    count,
                    ..
                } => {
                    if let Some(f) = self.flights.get_mut(shooter) {
                        if w.db.weapons.values().any(|d| {
                            d.name == *weapon && d.kind == harpoon_core::db::WeaponKind::Ascm
                        }) {
                            f.actual_asm_rounds += *count as u32;
                        }
                    }
                }
                EventKind::AirShotDown { unit, .. } | EventKind::Ditched { unit, .. } => {
                    explicit.insert(*unit);
                    if let Some(f) = self.flights.get_mut(unit) {
                        f.fate = if matches!(&e.kind, EventKind::Ditched { .. }) {
                            "ditched"
                        } else {
                            "shot_down"
                        }
                        .into();
                        f.ended_at = Some(e.tick);
                    }
                }
                EventKind::AirRecovered {
                    side,
                    carrier,
                    class,
                    count,
                } => {
                    *recoveries
                        .entry((side.index(), class.clone(), *carrier))
                        .or_default() += *count as u32;
                }
                _ => {}
            }
        }
        for ((side, class, carrier), count) in recoveries {
            let candidates: Vec<_> = self
                .flights
                .values()
                .filter(|f| {
                    f.side.index() == side
                        && f.fate == "airborne"
                        && !explicit.contains(&f.unit)
                        && w.db
                            .platforms
                            .get(&f.class)
                            .is_some_and(|d| d.name == class)
                        && w.entity(carrier)
                            .is_some_and(|h| f.last_pos.range_to(h.pos) <= 2.0)
                        && w.entity(f.unit).is_none_or(|u| !u.alive)
                })
                .map(|f| f.unit)
                .collect();
            if candidates.len() == count as usize {
                for id in candidates {
                    let f = self.flights.get_mut(&id).unwrap();
                    f.fate = "recovered".into();
                    f.ended_at = Some(w.tick);
                }
            } else {
                self.ambiguous_recovery_receipts += count;
            }
        }
        for f in self.flights.values_mut().filter(|f| f.fate == "airborne") {
            if w.entity(f.unit).is_none_or(|u| !u.alive) {
                f.fate = "unresolved_loss_or_recovery".into();
                f.ended_at = Some(w.tick);
            }
        }
        if w.tick % 30 == 0 {
            let mut measured = BTreeSet::new();
            for mem in memory {
                if let Some(bindings) = mem["supervision"]["ew"].as_object() {
                    for (id, b) in bindings {
                        let Some(jammer) = id
                            .parse::<u32>()
                            .ok()
                            .and_then(|id| w.entity(EntityId(id)))
                            .filter(|u| {
                                u.alive && harpoon_core::rules::electronic_warfare::emitting(u)
                            })
                        else {
                            continue;
                        };
                        for id in b["members"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_u64)
                        {
                            let Some(target) = w.entity(EntityId(id as u32)).filter(|u| u.alive)
                            else {
                                continue;
                            };
                            for radar in w
                                .entities
                                .values()
                                .filter(|u| u.alive && u.side != jammer.side&&harpoon_core::sensors::radar_emitting(u)&&u.systems.radar>0.05)
                            {
                                for sensor in &radar.sensors {
                                    let Some(def) = w.db.sensors.get(&sensor.sensor) else {
                                        continue;
                                    };
                                    if !measured.insert((radar.id,target.id,sensor.sensor.clone()))||harpoon_core::rules::electronic_warfare::combined_modifier(w,radar,def,target).is_none(){continue;}
                                    let multiplier =
                                        harpoon_core::rules::electronic_warfare::range_modifier(
                                            w, radar, def, target,
                                        );
                                    self.ew_checks += 1;
                                    if multiplier < 1.0 {
                                        self.effective_ew_checks += 1;
                                        self.ew_trace.push(json!({"tick":w.tick,"side":jammer.side,"support":jammer.id,"radar":radar.id,"protected_member":target.id,"modifier":multiplier,"note":"combined offensive-noise effect; not a marginal kill attribution"}));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

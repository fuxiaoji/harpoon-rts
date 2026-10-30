//! §9.4.7 optional, source-qualified first-round conditions.
//! Scenario rosters/fit declarations never create observer reports or FCS.
use super::*;
use crate::types::PlatformKind;
use crate::Side;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub source: String,
    pub enabled: bool,
    pub starting_speed: bool,
    pub astern: bool,
    pub surprise: bool,
    pub units: BTreeMap<String, UnitFit>,
    pub groups: BTreeMap<String, Group>,
    pub sun: Option<Sun>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitFit {
    pub source: String,
    pub single_seat_fighter: Option<bool>,
    pub rwr: Option<bool>,
    pub es: Option<bool>,
    pub attack_method: Option<AttackMethod>,
    #[serde(default)]
    pub attempt_surprise: bool,
    #[serde(default)]
    pub out_of_sun: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackMethod {
    PassiveGun,
    RadarGunRanging,
    IrMissile,
    RadarWeapon,
}
impl AttackMethod {
    fn radar(self) -> bool {
        matches!(self, Self::RadarGunRanging | Self::RadarWeapon)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub source: String,
    pub units: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sun {
    pub source: String,
    pub period: Period,
    pub position_roll_policy: Option<SunPolicy>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Morning,
    Afternoon,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SunPolicy {
    D100Percent,
    LiteralD10Decile,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub config: Configuration,
    pub fingerprint: u64,
    pub classes: BTreeMap<EntityId, String>,
    pub fits: BTreeMap<EntityId, UnitFit>,
    pub groups: Vec<BTreeSet<EntityId>>,
    pub episodes: BTreeMap<u64, Episode>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub pos: crate::Vec2,
    pub course_deg: f64,
    pub speed_kts: f64,
    pub altitude_m: f64,
    pub loiter: bool,
    pub starting_speed_kts: Option<f64>,
    pub astern_difference: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Episode {
    pub initiated_by: EntityId,
    pub attackers: BTreeSet<EntityId>,
    pub defenders: BTreeSet<EntityId>,
    pub entries: BTreeMap<EntityId, Entry>,
    pub first_round_at: i64,
    pub free_until: i64,
    pub surprised: bool,
    pub result: Value,
}
fn fingerprint(c: &Configuration) -> u64 {
    serde_json::to_vec(c)
        .unwrap()
        .iter()
        .fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
        })
}
fn number(key: &str) -> f64 {
    tables()["special_starting_conditions"][key]
        .as_f64()
        .expect("source9.4.7 parameter")
}
fn unique(w: &World, name: &str) -> Result<EntityId, String> {
    let rows: Vec<_> = w.entities.values().filter(|e| e.name == name).collect();
    if rows.len() != 1 || rows[0].kind != PlatformKind::Aircraft {
        return Err("9.4.7 exact unique aircraft declaration required".into());
    }
    Ok(rows[0].id)
}
pub(super) fn initialize(w: &mut World) -> Result<(), String> {
    let Some(c) = w.context.aircombat_2026.clone().filter(|c| c.enabled) else {
        return Ok(());
    };
    if !active(w) || c.source.trim().is_empty() {
        return Err("9.4.7 explicit Full-profile optional agreement/source required".into());
    }
    let mut s = State {
        fingerprint: fingerprint(&c),
        config: c.clone(),
        classes: BTreeMap::new(),
        fits: BTreeMap::new(),
        groups: Vec::new(),
        episodes: BTreeMap::new(),
    };
    for (name, fit) in &c.units {
        let id = unique(w, name)?;
        if fit.source.trim().is_empty() {
            return Err("9.4.7 crew/equipment/attack declaration source required".into());
        }
        s.classes.insert(id, w.entity(id).unwrap().class.clone());
        s.fits.insert(id, fit.clone());
    }
    let mut grouped = BTreeSet::new();
    for (key, g) in &c.groups {
        if key.trim().is_empty() || g.source.trim().is_empty() || g.units.is_empty() {
            return Err("9.4.7 sourced nonempty formation roster required".into());
        }
        let mut ids = BTreeSet::new();
        let mut side = None;
        for name in &g.units {
            let id = unique(w, name)?;
            let e = w.entity(id).unwrap();
            if !ids.insert(id) || !grouped.insert(id) || side.is_some_and(|s| s != e.side) {
                return Err("9.4.7 actual group must be unique, nonoverlapping and one side".into());
            }
            side = Some(e.side);
            s.classes.insert(id, e.class.clone());
        }
        s.groups.push(ids);
    }
    if c.sun
        .as_ref()
        .is_some_and(|sun| sun.source.trim().is_empty())
    {
        return Err("9.4.7 explicit sun-period source required".into());
    }
    w.rules
        .aircombat
        .as_mut()
        .ok_or("9.4.7 runtime unavailable")?
        .starting = Some(s);
    Ok(())
}
fn state(w: &World) -> Option<&State> {
    w.rules.aircombat.as_ref()?.starting.as_ref()
}
fn checked(w: &World) -> Result<Option<&State>, String> {
    let Some(s) = state(w) else { return Ok(None) };
    if w.context
        .aircombat_2026
        .as_ref()
        .is_none_or(|c| fingerprint(c) != s.fingerprint)
    {
        return Err("9.4.7 initial source agreement changed; reinitialize the scenario".into());
    }
    Ok(Some(s))
}
pub(super) fn members(
    w: &World,
    actor: EntityId,
    target: EntityId,
) -> Result<Vec<EntityId>, String> {
    let Some(s) = checked(w)? else {
        return Ok(vec![actor, target]);
    };
    let mut ids = BTreeSet::new();
    for lead in [actor, target] {
        if let Some(group) = s.groups.iter().find(|g| g.contains(&lead)) {
            ids.extend(group.iter().copied());
        } else {
            ids.insert(lead);
        }
    }
    let a = w.entity(actor).ok_or("9.4.7 actor unavailable")?;
    let mut radius = 0.0f64;
    for id in &ids {
        let e = w
            .entity(*id)
            .filter(|e| e.alive && e.kind == PlatformKind::Aircraft)
            .ok_or("9.4.7 formation member unavailable; no guessed subgroup")?;
        if s.classes.get(id).is_some_and(|class| class != &e.class) {
            return Err("9.4.7 original formation body changed class".into());
        }
        profile(&e.class)?;
        target_maneuver(w, *id)?;
        radius = radius.max(radius_for_fastest(max_speed(&e.class)?)?);
        let (aa, b) = (altitude_band(a.altitude_m), altitude_band(e.altitude_m));
        if b <= 1 || aa + 1 < b || aa > b + 2 {
            return Err("9.4.7 formation outside source altitude declaration domain".into());
        }
        if in_dogfight(w, *id) && *id != target {
            return Err("9.4.7 roster member already in another fight".into());
        }
    }
    if ids
        .iter()
        .any(|id| w.entity(*id).unwrap().pos.range_to(a.pos) > radius)
    {
        return Err("9.4.7 formation outside actual source circle; no guessed subgroup".into());
    }
    Ok(ids.into_iter().collect())
}
fn behind(target: &Entry, other: &Entry) -> bool {
    target.pos.range_to(other.pos) > 1e-9
        && crate::geom::angle_diff(target.course_deg, target.pos.bearing_to(other.pos)).abs()
            >= 90.0
}
fn native_track(w: &World, observer: EntityId, target: EntityId, continuous: bool) -> bool {
    let Some(o) = w.entity(observer).filter(|e| e.alive) else {
        return false;
    };
    let Some(contact) = w
        .find_contact(o.side, target)
        .and_then(|id| w.sides[o.side.index()].contacts.get(&id))
        .filter(|c| !c.stale)
    else {
        return false;
    };
    let _ = contact;
    w.rules.aaw.as_ref().is_some_and(|a| {
        a.firecontrol.tracks.values().any(|t| {
            t.observer == observer
                && t.target == target
                && t.last <= w.tick
                && w.tick - t.last
                    <= if continuous {
                        number("continuous_track_phase_age_s") as i64
                    } else {
                        number("awareness_report_age_s") as i64
                    }
                && t.report
                    .as_ref()
                    .is_some_and(|r| r.observer == observer && r.target == target && r.at == t.last)
                && o.sensors
                    .iter()
                    .any(|s| s.sensor == t.sensor && !crate::sensors::sensor_damaged(o, &s.sensor))
                && w.rules.communications.as_ref().is_some_and(|_| {
                    w.find_contact(o.side, target).is_some_and(|c| {
                        super::super::communications2026::native_report_view(
                            w, observer, c, &t.sensor,
                        )
                        .is_ok()
                    })
                })
        })
    })
}
fn friendly_track(
    w: &World,
    defenders: &BTreeSet<EntityId>,
    attackers: &BTreeSet<EntityId>,
) -> bool {
    defenders.iter().any(|defender| {
        let Some(d) = w.entity(*defender) else {
            return false;
        };
        w.entities
            .values()
            .filter(|u| u.alive && u.side == d.side && u.id != *defender)
            .any(|u| {
                if !attackers.iter().any(|a| native_track(w, u.id, *a, true)) {
                    return false;
                }
                let Some(fits) = w
                    .context
                    .communications_2026
                    .as_ref()
                    .and_then(|c| c.units.get(&u.name))
                else {
                    return false;
                };
                fits.fits.keys().any(|key| {
                    super::super::communications2026::validate_transport(
                        w,
                        d.side,
                        u.id,
                        key,
                        &[*defender],
                    )
                    .is_ok()
                })
            })
    })
}
fn count_band(count: usize) -> usize {
    match count {
        0 | 1 => 0,
        2 => 1,
        3 | 4 => 2,
        5..=8 => 3,
        _ => 4,
    }
}
fn source_chance(a: usize, d: usize, multi: bool) -> Result<f64, String> {
    let col = count_band(d) + usize::from(multi);
    tables()["surprise"]
        .get(count_band(a))
        .and_then(|r| r.get(col))
        .and_then(Value::as_f64)
        .ok_or_else(|| "9.4.7 non-single-seat shift exceeds last printed9+ column".into())
}
fn radar_guidance(weapon: &str) -> Option<bool> {
    let guidance = super::super::aam2026::tables()["weapons"]
        .get(weapon)?
        .get("guidance")?
        .as_str()?;
    match guidance {
        "IRH" | "TIRH" | "TIIRH" | "IIRH" => Some(false),
        "SARH" | "ARH" | "TARH" => Some(true),
        _ => None,
    }
}
fn method_ready(w: &World, id: EntityId, method: AttackMethod) -> bool {
    let Some(e) = w.entity(id).filter(|e| e.alive) else {
        return false;
    };
    match method {
        AttackMethod::PassiveGun | AttackMethod::RadarGunRanging => {
            profile(&e.class)
                .ok()
                .is_some_and(|p| p["gun"].as_f64().is_some_and(|g| g > 0.0))
                && (method == AttackMethod::PassiveGun
                    || e.systems.radar > 0.05
                        && e.sensors.iter().any(|s| {
                            w.db.sensors
                                .get(&s.sensor)
                                .is_some_and(|d| d.kind == crate::db::SensorKind::Radar)
                                && !crate::sensors::sensor_damaged(e, &s.sensor)
                        }))
        }
        AttackMethod::IrMissile | AttackMethod::RadarWeapon => e.weapons.iter().any(|m| {
            m.ammo > 0
                && w.db
                    .weapons
                    .get(&m.weapon)
                    .is_some_and(|d| d.kind == crate::db::WeaponKind::Aam)
                && radar_guidance(&m.weapon) == Some(method == AttackMethod::RadarWeapon)
        }),
    }
}
pub(super) fn begin(
    w: &mut World,
    key: u64,
    actor: EntityId,
    members: &[EntityId],
    first_round_at: i64,
) -> Result<(), String> {
    let Some(s) = checked(w)?.cloned() else {
        return Ok(());
    };
    if s.episodes.contains_key(&key) {
        return Ok(());
    }
    let side = w.entity(actor).ok_or("9.4.7 actor unavailable")?.side;
    let attackers: BTreeSet<_> = members
        .iter()
        .copied()
        .filter(|id| w.entity(*id).is_some_and(|e| e.side == side))
        .collect();
    let defenders: BTreeSet<_> = members
        .iter()
        .copied()
        .filter(|id| !attackers.contains(id))
        .collect();
    let mut entries = BTreeMap::new();
    for id in members {
        let e = w.entity(*id).ok_or("9.4.7 member unavailable")?;
        let loiter = w
            .rules
            .aviation
            .as_ref()
            .and_then(|s| s.flights.get(id))
            .is_some_and(|f| f.throttle == super::super::aviation2026::Throttle::Loiter);
        let speed = if !s.config.starting_speed {
            None
        } else if loiter {
            Some(
                number("loiter_cruise_fraction")
                    * super::super::aviation2026::entity_source_speed(
                        w,
                        *id,
                        e.altitude_m,
                        super::super::aviation2026::Throttle::Cruise,
                    )?,
            )
        } else {
            Some(e.speed_kts)
        };
        entries.insert(
            *id,
            Entry {
                pos: e.pos,
                course_deg: e.course_deg,
                speed_kts: e.speed_kts,
                altitude_m: e.altitude_m,
                loiter,
                starting_speed_kts: speed,
                astern_difference: 0.0,
            },
        );
    }
    if s.config.astern {
        for id in members {
            let e = w.entity(*id).unwrap();
            let enemies: Vec<_> = members
                .iter()
                .filter(|other| w.entity(**other).unwrap().side != e.side)
                .copied()
                .collect();
            let own_rear = enemies
                .iter()
                .any(|other| behind(&entries[id], &entries[other]));
            let astern = !enemies.is_empty()
                && enemies
                    .iter()
                    .all(|other| behind(&entries[other], &entries[id]));
            entries.get_mut(id).unwrap().astern_difference = if own_rear {
                -number("enemy_astern_difference_penalty")
            } else if astern {
                number("astern_difference_bonus")
            } else {
                0.0
            };
        }
    }
    let mut result = json!({"fight":key,"source":"English2026 PDF103/104 §9.4.7",
        "attacking_count":attackers.len(),"defending_count":defenders.len(),
        "chance_percent":null,"d100":null,"surprised":false,"immunity":null,"pending":null,
        "sun":null,"entries":entries,"rts_mapping":"declared physical rosters and pre-circle snapshot; first60s attack window after first position resolution"});
    let attempt = s.config.surprise && s.fits.get(&actor).is_some_and(|f| f.attempt_surprise);
    result["attempted"] = json!(attempt);
    let mut pending = None;
    let mut immunity = None;
    let mut chance = None;
    let mut surprised = false;
    if attempt {
        if !attackers
            .iter()
            .all(|a| defenders.iter().all(|d| behind(&entries[d], &entries[a])))
        {
            immunity = Some("not_astern");
        } else if defenders
            .iter()
            .any(|d| attackers.iter().any(|a| native_track(w, *d, *a, false)))
        {
            immunity = Some("detected_by_defender");
        } else if friendly_track(w, &defenders, &attackers) {
            immunity = Some("communicating_friendly_track");
        }
        if immunity.is_none()
            && (!s.groups.iter().any(|g| g == &attackers)
                || !s.groups.iter().any(|g| g == &defenders))
        {
            pending=Some("9.4.7 actual attacking/defending formation roster is not explicitly sourced; no guessed single pair denominator".into());
        }
        let methods: Option<Vec<_>> = attackers
            .iter()
            .map(|a| s.fits.get(a).and_then(|f| f.attack_method))
            .collect();
        if immunity.is_none() {
            if attackers.iter().any(|a| {
                s.fits
                    .get(a)
                    .and_then(|f| f.attack_method)
                    .is_some_and(|m| !method_ready(w, *a, m))
            }) {
                pending=Some("9.4.7 declared first-round attack method lacks its actual fitted functional gun/radar/source AAM inventory".into());
            }
            match methods {
                None => pending = Some("9.4.7 first-round attack method unknown".to_string()),
                Some(ref methods) if methods.iter().any(|m| m.radar()) => {
                    if defenders.iter().any(|d| {
                        s.fits
                            .get(d)
                            .is_some_and(|f| f.rwr == Some(true) || f.es == Some(true))
                    }) {
                        immunity = Some("radar_warning_receiver");
                    } else if defenders.iter().any(|d| {
                        s.fits
                            .get(d)
                            .is_none_or(|f| f.rwr.is_none() || f.es.is_none())
                    }) {
                        pending = Some("9.4.7 RWR/ES fit unknown for a radar attack".into());
                    }
                }
                _ => {}
            }
        }
        if immunity.is_none() && pending.is_none() {
            let crew: Option<Vec<_>> = defenders
                .iter()
                .map(|d| s.fits.get(d).and_then(|f| f.single_seat_fighter))
                .collect();
            if let Some(crew) = crew {
                let single = crew.iter().filter(|v| **v).count();
                if single * 2 == crew.len() {
                    pending = Some(
                        "9.4.7 mixed defenders have50:50 crew types and no source majority".into(),
                    );
                } else {
                    match source_chance(attackers.len(), defenders.len(), single * 2 < crew.len()) {
                        Ok(p) => chance = Some(p),
                        Err(e) => pending = Some(e),
                    }
                }
            } else {
                pending = Some("9.4.7 defending single-seat/multicrew type unknown".into());
            }
        }
        if immunity.is_none()
            && pending.is_none()
            && s.fits.get(&actor).is_some_and(|f| f.out_of_sun)
        {
            match &s.config.sun {
                None => pending = Some("9.4.7 sun period/positioning policy not declared".into()),
                Some(sun) => {
                    let desired = match sun.period {
                        Period::Morning => 90.0,
                        Period::Afternoon => 270.0,
                    };
                    let eligible = w.day_night == crate::types::DayNight::Day
                        && attackers.iter().all(|a| {
                            defenders.iter().all(|d| {
                                crate::geom::angle_diff(
                                    entries[d].pos.bearing_to(entries[a].pos),
                                    desired,
                                )
                                .abs()
                                    < 1e-9
                                    && crate::geom::angle_diff(
                                        entries[a].course_deg,
                                        (desired + 180.0) % 360.0,
                                    )
                                    .abs()
                                        < 1e-9
                            })
                        });
                    let mut sun_result = json!({"eligible":eligible,"positioned":false,"chance_percent":source_chance(attackers.len(),1,false)?,"roll":null,"roll_sides":null,"policy":sun.position_roll_policy});
                    if eligible {
                        if let Some(policy) = sun.position_roll_policy {
                            let sides = match policy {
                                SunPolicy::D100Percent => 100,
                                SunPolicy::LiteralD10Decile => 10,
                            };
                            let die = roll(w, actor, &format!("9.4.7.sun_position.{key}"), sides);
                            let percent = if sides == 10 {
                                f64::from(die) * 10.0
                            } else {
                                f64::from(die)
                            };
                            let positioned = percent <= source_chance(attackers.len(), 1, false)?;
                            sun_result["roll"] = json!(die);
                            sun_result["roll_sides"] = json!(sides);
                            sun_result["positioned"] = json!(positioned);
                            if positioned {
                                chance = chance.map(|p| p + number("sun_bonus_percent"));
                            }
                        } else {
                            pending=Some("9.4.7 sun prints D10 against non-decile percentages; explicit positioning policy required".into());
                        }
                    }
                    result["sun"] = sun_result;
                }
            }
        }
        if immunity.is_none() && pending.is_none() {
            let die = roll(w, actor, &format!("9.4.7.surprise.{key}"), 100);
            result["d100"] = json!(die);
            surprised = chance.is_some_and(|p| f64::from(die) <= p);
        }
    }
    result["chance_percent"] = json!(chance);
    result["surprised"] = json!(surprised);
    result["immunity"] = json!(immunity);
    result["pending"] = json!(pending);
    let episode = Episode {
        initiated_by: actor,
        attackers,
        defenders,
        entries,
        first_round_at,
        free_until: first_round_at + tables()["round_seconds"].as_i64().unwrap(),
        surprised,
        result: result.clone(),
    };
    w.rules
        .aircombat
        .as_mut()
        .unwrap()
        .starting
        .as_mut()
        .unwrap()
        .episodes
        .insert(key, episode);
    w.emit(
        EventKind::RulesEvent {
            domain: "aircombat".into(),
            rule: "9.4.7.starting".into(),
            side: Some(side),
            unit: Some(actor),
            contact: None,
            detail: result.to_string(),
        },
        Visibility::Referee,
    );
    Ok(())
}
fn episode_for(w: &World, id: EntityId) -> Option<&Episode> {
    state(w)?.episodes.iter().rev().find_map(|(key, e)| {
        w.rules
            .aircombat
            .as_ref()?
            .fights
            .get(key)
            .filter(|f| f.members.contains(&id))
            .map(|_| e)
    })
}
pub(super) fn has_episode(w: &World, id: EntityId) -> bool {
    episode_for(w, id).is_some()
}
pub(super) fn surprised(w: &World, id: EntityId) -> bool {
    episode_for(w, id).is_some_and(|e| {
        e.surprised
            && e.defenders.contains(&id)
            && w.tick >= e.first_round_at
            && w.tick < e.free_until
    })
}
pub(super) fn round_speed(w: &World, id: EntityId, round: u8, normal: f64) -> f64 {
    let speed = if round == 1 {
        episode_for(w, id)
            .and_then(|e| e.entries.get(&id))
            .and_then(|e| e.starting_speed_kts)
            .unwrap_or(normal)
    } else {
        normal
    };
    speed.min(
        super::super::expanded_damage2026::aircraft_speed_cap(w, id)
            .ok()
            .flatten()
            .unwrap_or(f64::INFINITY),
    )
}
pub(super) fn astern(w: &World, id: EntityId, round: u8) -> f64 {
    if round == 1 {
        episode_for(w, id)
            .and_then(|e| e.entries.get(&id))
            .map(|e| e.astern_difference)
            .unwrap_or(0.0)
    } else {
        0.0
    }
}
pub(super) fn reduced_maneuver(mr: f64) -> f64 {
    (mr - number("surprised_maneuver_reduction")).max(number("surprised_maneuver_floor"))
}
fn first_attack_method(w: &World, id: EntityId) -> Option<AttackMethod> {
    let e = episode_for(w, id)?;
    if e.result["attempted"] != true
        || !e.attackers.contains(&id)
        || w.tick < e.first_round_at
        || w.tick >= e.free_until
    {
        return None;
    }
    state(w)?.fits.get(&id)?.attack_method
}
pub(super) fn gun_allowed(w: &World, id: EntityId) -> bool {
    match first_attack_method(w, id) {
        Some(AttackMethod::IrMissile | AttackMethod::RadarWeapon) => false,
        Some(AttackMethod::RadarGunRanging) => w
            .entity(id)
            .is_some_and(|e| crate::sensors::radar_operational(&w.db, e)),
        _ => true,
    }
}
pub(super) fn weapon_gate(w: &World, id: EntityId, weapon: &str) -> Result<(), String> {
    let Some(method) = first_attack_method(w, id) else {
        return Ok(());
    };
    let d =
        w.db.weapons
            .get(weapon)
            .ok_or("9.4.7 actual first-round weapon unavailable")?;
    let allowed = d.kind == crate::db::WeaponKind::Aam
        && match method {
            AttackMethod::IrMissile => radar_guidance(weapon) == Some(false),
            AttackMethod::RadarWeapon => radar_guidance(weapon) == Some(true),
            _ => false,
        };
    if !allowed {
        return Err("9.4.7 actual release conflicts with the explicit first-round surprise method; no free radar/IR substitution".into());
    }
    Ok(())
}
pub(super) fn own_view(w: &World, id: EntityId) -> Option<Value> {
    let e = episode_for(w, id)?;
    Some(
        json!({"optional_agreed":true,"first_round_at":e.first_round_at,"first_window_ends_at":e.free_until,
      "first_window_active":w.tick>=e.first_round_at&&w.tick<e.free_until,"surprised":surprised(w,id)}),
    )
}

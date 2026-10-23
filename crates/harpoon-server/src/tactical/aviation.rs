//! Aviation tactics using existing player commands. Distances/timers below
//! are policy parameters, not Harpoon adjudication constants.
use super::*;
use harpoon_core::{geom::Vec2, observe::ContactObs};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Phase {
    Assemble,
    Ingress,
    Reacquire,
    Attack,
    Egress,
    Patrol,
    Intercept,
    Protect,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct AirMind {
    home: EntityId,
    born: i64,
    phase: Phase,
    target: Option<ContactId>,
    assembly: Vec2,
    flank: Vec2,
    #[serde(default)]
    attack_since: Option<i64>,
    #[serde(default)]
    last_asw_retask: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    relay_until: Option<i64>,
}

/// A friendly launch cohort shares one assignment and one radar relay. A
/// fresh contact does not immediately order the observer down again.
#[derive(Clone,Serialize,Deserialize)]
pub(super) struct Package {
    home:EntityId,born:i64,members:Vec<EntityId>,target:Option<ContactId>,
    searcher:Option<EntityId>,stage:String,changed_at:i64,
}

impl TacticalStaff {
    pub(super) fn note(
        &mut self,
        unit: EntityId,
        contact: Option<ContactId>,
        action: &str,
        zh: String,
        en: String,
    ) {
        let key = (action.to_string(), contact);
        if self.note_keys.get(&unit) == Some(&key) {
            return;
        }
        self.note_keys.insert(unit, key);
        self.notes.push(StaffNote {
            unit,
            contact,
            action: action.into(),
            zh,
            en,
        });
    }

    fn home<'a>(&self, obs: &'a Observation, u: &UnitObs) -> Option<&'a UnitObs> {
        self.air_minds
            .get(&u.id)
            .and_then(|m| obs.units.iter().find(|h| h.id == m.home))
            .or_else(||self.supervision.known_origin(u.id).and_then(|id|obs.units.iter().find(|h|h.id==id)))
            .or_else(|| {
                obs.units
                    .iter()
                        .filter(|h| matches!(self.role_for(h),Role::Carrier|Role::Airbase))
                    .min_by(|a, b| u.pos.range_to(a.pos).total_cmp(&u.pos.range_to(b.pos)))
            })
            .or_else(|| self.anchor_of(obs))
    }

    fn nav(cmds: &mut Vec<Command>, u: &UnitObs, point: Vec2) {
        cmds.push(Command::ClearStation { unit: u.id });
        // Waypoints are consumed by the existing movement loop; raw course
        // orders alone are overwritten by the standing air mission next tick.
        cmds.push(Command::Goto {
            unit: u.id,
            waypoints: vec![point],
        });
    }

    fn orbit(cmds: &mut Vec<Command>, u: &UnitObs, point: Vec2, radius: f64) {
        let goal = if u.pos.range_to(point) < radius * 1.5 {
            point.advanced(point.bearing_to(u.pos) + 40.0, radius)
        } else {
            point
        };
        Self::nav(cmds, u, goal);
    }

    fn air_threat<'a>(
        &self,
        obs: &'a Observation,
        pos: Vec2,
        radius: f64,
    ) -> Option<&'a ContactObs> {
        obs.contacts
            .iter()
            .filter(|c| {
                c.category == PlatformKind::Aircraft
                    && !c.stale
                    && !c.bearing_only
                    && c.quality > 0.4
                    && pos.range_to(c.estimate) <= radius
            })
            .min_by(|a, b| {
                pos.range_to(a.estimate)
                    .total_cmp(&pos.range_to(b.estimate))
            })
    }

    fn surface_danger(&self, obs: &Observation, point: Vec2, margin: f64) -> bool {
        obs.contacts.iter().any(|c| {
            c.category == PlatformKind::Ship
                && !c.stale
                && !self.sensor_contact(c)
                && c.quality > 0.25
                && point.range_to(c.estimate) < margin
        })
    }

    fn observed_missile_danger(&self,obs:&Observation,point:Vec2)->bool {
        obs.contacts.iter().any(|c|c.category==PlatformKind::Missile && !c.stale
            && !c.bearing_only && c.quality>0.0 && point.range_to(c.estimate)<45.0)
    }

    fn loaded_range(&self, u: &UnitObs, kind: WeaponKind) -> f64 {
        u.weapons
            .iter()
            .filter(|w| w.ammo > 0 && self.kind_of(&w.weapon) == Some(kind))
            .map(|w| self.range_of(&w.weapon))
            .fold(0.0, f64::max)
    }
    fn relay_covers(&self,u:&UnitObs,target:&ContactObs)->bool {
        u.sensors.iter().any(|s|s.active&&s.vs_surface.is_some_and(|r|u.pos.range_to(target.estimate)<r.min(harpoon_core::geom::horizon_nm(u.altitude_m,0.0))*0.9))
    }

    pub(super) fn manage_aviation(&mut self, obs: &Observation, cmds: &mut Vec<Command>) {
        let carriers: Vec<_> = obs
            .units
            .iter()
            .filter(|u| matches!(self.role_for(u),Role::Carrier|Role::Airbase))
            .collect();
        // Record the launch cohort from friendly observations only. Never read
        // enemy entities, mission target truth, fuel or hidden weapon loadouts.
        self.air_minds
            .retain(|id, _| obs.units.iter().any(|u| u.id == *id));
        for u in obs
            .units
            .iter()
            .filter(|u| u.kind == PlatformKind::Aircraft)
        {
            if self.air_minds.contains_key(&u.id) {
                continue;
            }
            let Some(home) = self.home(obs, u) else {
                continue;
            };
            let opening=obs.exercise.as_ref().and_then(|e|e.opening_flights.iter().find(|f|f.unit==u.id));
            let home=opening.and_then(|f|obs.units.iter().find(|h|h.id==f.home)).unwrap_or(home);
            let target = opening.map(|f|f.target).or_else(||self
                .wave_targets
                .get(&home.id)
                .copied()
                .or_else(|| self.best_ship_target(obs, home).map(|(id, _)| id)));
            let target_pos = target
                .map(|id| contact_pos(obs, id, home.pos))
                .unwrap_or_else(|| home.pos.advanced(self.threat_axis_deg, 150.0));
            let axis = home.pos.bearing_to(target_pos);
            let sign = if home.id.0 % 2 == 0 { 1.0 } else { -1.0 };
            let assembly = home.pos.advanced(axis + sign * 70.0, 12.0);
            let range = self.loaded_range(u, WeaponKind::Ascm);
            let flank = target_pos.advanced(axis + 180.0 + sign * 25.0, range.max(35.0) * 1.15);
            self.air_minds.insert(
                u.id,
                AirMind {
                    home: home.id,
                    born: if opening.is_some(){obs.tick-360}else{obs.tick},
                    phase: if self.role_for(u) == Role::Striker {
                        if opening.is_some(){Phase::Ingress}else{Phase::Assemble}
                    } else {
                        Phase::Patrol
                    },
                    target,
                    assembly,
                    flank,
                    attack_since: None,
                    last_asw_retask: -100000,
                    relay_until: None,
                },
            );
        }

        // One launch request per carrier per decision. Ready inventory is
        // decremented locally by choosing a single task, so we cannot double
        // book the same fighters for CAP and strike in a planning window.
        for home in carriers {
            let Some(deck) = &home.deck else {
                continue;
            };
            if !deck.can_launch{continue;}
            if obs.tick - self.last_launch.get(&home.id).copied().unwrap_or(-100000) < 60 {
                continue;
            }
            let local_air: Vec<_> = obs
                .units
                .iter()
                .filter(|u| {
                    u.kind == PlatformKind::Aircraft
                        && u.mission.as_deref() != Some("rtb")
                        && self.air_minds.get(&u.id).is_some_and(|m| m.home == home.id)
                })
                .collect();
            let cap_count = local_air
                .iter()
                .filter(|u| {
                    self.role_for(u) == Role::Cap
                        && u.endurance_s.unwrap_or(10000.0) > self.params.cap_fuel_rtb_s
                        && self.loaded_range(u, WeaponKind::Aam) > 0.0
                })
                .count() as u32;
            let cap_goal = self.params.cap_min_size.max(2);
            let cap_class = deck
                .ready
                .iter()
                .find(|(c, n)| *n > 0 && self.has_kind(c, WeaponKind::Aam) && harpoon_core::rules::electronic_warfare::capability(c).is_none());
            let aew = local_air.iter().any(|u| self.role_for(u) == Role::Aew && u.endurance_s.unwrap_or(0.0)>u.pos.range_to(home.pos)/self.cruise_of(&u.class)*3600.0*1.3+900.0);
            let asw = local_air
                .iter()
                .filter(|u| self.role_for(u) == Role::AswScreen)
                .count();
            let asw_area = self.db.as_ref().and_then(|db|super::assessment::asw_station(obs,db,home,asw as u32));
            let buoy_overlap = obs
                .units
                .iter()
                .filter(|u| {
                    u.class == harpoon_core::rules::asw::SONOBUOY_CLASS
                        && asw_area.is_some_and(|area|u.pos.range_to(area) < 10.0)
                })
                .count();
            let strike_count = local_air
                .iter()
                .filter(|u| {
                    self.role_for(u) == Role::Striker
                        && self.loaded_range(u, WeaponKind::Ascm) > 0.0
                })
                .count();
            let strike_target = obs
                .contacts
                .iter()
                .filter(|c| {
                    c.category == PlatformKind::Ship
                        && !self.sensor_contact(c)
                        && !c.stale
                        && !c.bearing_only
                        && c.quality >= self.params.strike_min_q
                        && self.supervision.target_allowed(c.id,obs.tick)
                        && deck.ready.iter().any(|(class,n)|*n>=2 && self.can_fly_anti_ship(class)
                            && home.pos.range_to(c.estimate)<=self.strike_sortie_reach(class))
                        && c.bda_hp_fraction.unwrap_or(1.0) > 0.0
                })
                .max_by(|a, b| {
                    self.target_value(a, home)
                        .total_cmp(&self.target_value(b, home))
                });
            let request = if cap_count < cap_goal && cap_class.is_some() {
                let (class, n) = cap_class.unwrap();
                Some((
                    class.clone(),
                    (*n).min((cap_goal - cap_count).min(2) as u8),
                    AirMission::Cap {
                        station: home.pos,
                        radius_nm: 15.0,
                        follow: true,
                    },
                    "cap_launch",
                    "补充舰队防空巡逻",
                    "Reinforce fleet CAP",
                ))
            } else if !aew && deck.ready.iter().any(|(c, n)| *n > 0 && is_aew_class(c)&&self.supervision.support_launch_allowed(obs,home,c)) {
                let (class, _) = deck
                    .ready
                    .iter()
                    .find(|(c, n)| *n > 0 && is_aew_class(c)&&self.supervision.support_launch_allowed(obs,home,c))
                    .unwrap();
                Some((
                    class.clone(),
                    1,
                    AirMission::Recon {
                        point: home.pos.advanced(self.threat_axis_deg + 180.0, 25.0),
                    },
                    "aew_launch",
                    "补充后方预警巡逻",
                    "Launch rearward AEW patrol",
                ))
            } else if asw < 2
                && asw_area.is_some()
                && buoy_overlap < 8
                && deck
                    .ready
                    .iter()
                    .any(|(c, n)| *n > 0 && self.can_fly_asw(c)&&self.supervision.support_launch_allowed(obs,home,c))
            {
                let (class, n) = deck
                    .ready
                    .iter()
                    .find(|(c, n)| *n > 0 && self.can_fly_asw(c)&&self.supervision.support_launch_allowed(obs,home,c))
                    .unwrap();
                let area = asw_area.unwrap();
                Some((
                    class.clone(),
                    (*n).min((2 - asw) as u8),
                    AirMission::Asw {
                        area,
                        radius_nm: 6.0,
                    },
                    "asw_launch",
                    "在舰队防空保护内布置反潜搜索",
                    "Launch ASW search inside fleet cover",
                ))
            } else if strike_count>0 && obs.radar_environment.is_some() && obs.weather.jam.iter().all(|v|*v==0.0) && !local_air.iter().any(|u|self.role_for(u)==Role::Ew)
                && deck.ready.iter().any(|(c,n)|*n>0 && harpoon_core::rules::electronic_warfare::capability(c).is_some()) {
                let (class,_)=deck.ready.iter().find(|(c,n)|*n>0 && harpoon_core::rules::electronic_warfare::capability(c).is_some()).unwrap();
                Some((class.clone(),1,AirMission::Recon {point:home.pos.advanced(self.threat_axis_deg,25.0)},"ew_launch","组织电子战支援，按挂载选择干扰模式","Launch fitted electronic-attack support"))
            } else if cap_count>=cap_goal && cap_class.is_some() && (strike_target.is_none()||strike_count>0)
                && !local_air.iter().any(|u|self.role_for(u)==Role::Scout && u.endurance_s.unwrap_or(10000.0)>1800.0) {
                let (class,_)=cap_class.unwrap();
                Some((class.clone(),1,AirMission::Recon {point:home.pos.advanced(self.threat_axis_deg,70.0)},"recon_launch","放飞前出侦察与航迹接替机，维持反舰目标定位","Launch forward scout / track relay while retaining CAP"))
            } else if strike_count == 0
                && cap_count >= cap_goal
                && obs.tick
                    - self
                        .last_strike_wave
                        .get(&home.id)
                        .copied()
                        .unwrap_or(-100000)
                    >= self.params.strike_wave_interval_s
                        .min(if self.posture==2 {360}else{i64::MAX})
            {
                strike_target.and_then(|target| {
                    deck.ready
                        .iter()
                        .find(|(c, n)| *n >= 2 && self.can_fly_anti_ship(c)
                            && home.pos.range_to(target.estimate)<=self.strike_sortie_reach(c))
                        .map(|(class, n)| {
                            (
                                class.clone(),
                                (*n).min(self.params.strike_wave_size.max(1) as u8),
                                AirMission::Strike { contact: target.id },
                                "strike_launch",
                                "组织反舰突击波；先集结下降，再侧翼超低空突防",
                                "Form anti-ship wave: assemble and descend before flanking ingress",
                            )
                        })
                })
            } else {
                None
            };
            if let Some((class, count, mission, action, zh, en)) = request {
                let contact = if let AirMission::Strike { contact } = mission {
                    Some(contact)
                } else {
                    None
                };
                let manual_strike = matches!(mission, AirMission::Strike { .. });
                let altitude_m = match mission {
                    AirMission::Strike { .. } => self.params.strike_altitude_m.clamp(21.0, 60.0),
                    AirMission::Cap { .. } => 3000.0,
                    AirMission::Asw { .. } => 500.0,
                    _ => 3000.0,
                };
                let waypoints=contact.and_then(|id|obs.contacts.iter().find(|c|c.id==id)).map(|c| {
                    let axis=home.pos.bearing_to(c.estimate);let sign=if home.id.0%2==0 {1.0}else{-1.0};
                    vec![home.pos.advanced(axis+sign*70.0,12.0)]
                }).unwrap_or_default();
                cmds.push(Command::LaunchAircraft {
                    unit: home.id,
                    class,
                    count,
                    mission,
                    profile: Some(harpoon_core::commands::LaunchProfile {waypoints,
                        altitude_m,
                        weapons_free: false,
                        manual_strike,
                        engagement: Some(harpoon_core::commands::LaunchEngagement {air_only:!manual_strike,max_in_flight:2}),
                    }),
                });
                self.note(
                    home.id,
                    contact,
                    action,
                    format!("{}：{}，下令{}架", home.name, zh, count),
                    format!("{}: {}; ordered {} aircraft", home.name, en, count),
                );
            }
        }

        self.refresh_strike_packages(obs);
        let ew_units:Vec<_>=obs.units.iter().filter(|u|self.role_for(u)==Role::Ew).collect();
        for ew in ew_units {
            // Jamming is inert until the pod is switched on (§12.5). Standoff
            // pods jam the whole horizon; spot pods need a current ES/RWR
            // detection of the victim radar, so fall back to escort when no
            // ESM-backed ship track is fresh enough.
            if ew.mission.as_deref()!=Some("rtb") {
                if let Some(jam)=harpoon_core::rules::electronic_warfare::capability(&ew.class) {
                    let esm_ships:Vec<ContactId>=obs.contacts.iter().filter(|c|
                        !c.stale && !c.bearing_only && c.quality>=0.4 && c.category==PlatformKind::Ship
                        && c.sensors.iter().any(|s| s.contains("esm"))).map(|c|c.id).collect();
                    let (mode,contacts)=if jam.spot_targets>0 && !esm_ships.is_empty() {
                        ("spot", esm_ships.into_iter().take(jam.spot_targets).collect())
                    } else if jam.standoff {("standoff", vec![])} else {("escort", vec![])};
                    cmds.push(Command::SetJamming {unit:ew.id,mode:mode.into(),contacts});
                }
            }
            let package=self.packages.values().filter(|p|!p.members.is_empty()).min_by(|a,b| {
                let distance=|p:&Package|p.members.iter().filter_map(|id|obs.units.iter().find(|u|u.id==*id)).map(|u|u.pos.range_to(ew.pos)).fold(f64::INFINITY,f64::min);
                distance(a).total_cmp(&distance(b))
            });
            if let (Some(package),Some(binding))=(package,self.supervision.ew.get_mut(&ew.id)) {
                binding.members=package.members.clone();binding.target=package.target;binding.until=obs.tick+600;
            }
        }
        for u in obs
            .units
            .iter()
            .filter(|u| u.kind == PlatformKind::Aircraft)
        {
            let Some(mut mind) = self.air_minds.get(&u.id).cloned() else {
                continue;
            };
            let Some(home) = self.home(obs, u) else {
                continue;
            };
            let home_pos = home.pos;
            if u.mission.as_deref() == Some("rtb") {
                if harpoon_core::rules::flight_profiles::supported(&u.class) {
                    cmds.push(Command::SetAltitude {unit:u.id,altitude_m:3000.0});
                    cmds.push(Command::SetThrottle {unit:u.id,setting:"cruise".into()});
                }
                if harpoon_core::rules::electronic_warfare::capability(&u.class).is_some() {cmds.push(Command::SetJamming {unit:u.id,mode:"off".into(),contacts:vec![]});}
                if mind.phase != Phase::Egress {
                    cmds.push(Command::Goto {
                        unit: u.id,
                        waypoints: vec![],
                    });
                    cmds.push(Command::ClearStation { unit: u.id });
                    cmds.push(Command::ClearTarget { unit: u.id });
                    mind.phase = Phase::Egress;
                    self.note(
                        u.id,
                        None,
                        "rtb",
                        format!("{}：清除突防航点，按引擎返航回收", u.name),
                        format!("{}: clear tactical route for engine RTB/recovery", u.name),
                    );
                }
                self.air_minds.insert(u.id, mind);
                continue;
            }
            match self.role_for(u) {
                Role::Striker => {
                    self.supervision.bind_strike(u.id,mind.target,obs.tick);
                    self.fly_strike(obs, u, &mut mind, cmds)
                },
                Role::Cap => {
                    // Use explicit air assignments so a multirole CAP does
                    // not expend its anti-ship load while holding the screen.
                    cmds.push(Command::SetWeaponsFree {
                        unit: u.id,
                        free: false,
                    });
                    let armed = self.loaded_range(u, WeaponKind::Aam) > 0.0;
                    let rested = u.endurance_s.unwrap_or(10000.0) > self.params.cap_fuel_rtb_s;
                    if !armed || !rested {
                        cmds.push(Command::ReturnToBase { unit: u.id });
                        self.note(
                            u.id,
                            None,
                            "cap_rtb",
                            format!("{}：弹药或油量不足，单机返航轮换", u.name),
                            format!(
                                "{}: ammunition/fuel reserve reached; individual RTB",
                                u.name
                            ),
                        );
                        continue;
                    }
                    let defended = obs
                        .units
                        .iter()
                        .filter(|a| {
                            a.kind == PlatformKind::Aircraft
                                && self.role_for(a) == Role::Striker
                                && a.mission.as_deref() != Some("rtb")
                                && home_pos.range_to(a.pos) < 90.0
                        })
                        .min_by(|a, b| u.pos.range_to(a.pos).total_cmp(&u.pos.range_to(b.pos)));
                    let center = defended.map(|a| a.pos).unwrap_or(home_pos);
                    let threat = self.air_threat(obs, center, 65.0).filter(|c| {
                        home_pos.range_to(c.estimate) <= self.params.cap_leash_nm
                            || defended.is_some_and(|a| a.pos.range_to(c.estimate) < 30.0)
                    });
                    let threat = threat.filter(|c| !self.surface_danger(obs, c.estimate, 40.0));
                    if let Some(c) = threat.filter(|_| armed && rested) {
                        let range = self.loaded_range(u, WeaponKind::Aam);
                        // Keep an AAM standoff; do not pursue all the way to
                        // the enemy's deck or sacrifice CAP on an AEW decoy.
                        let goal = c
                            .estimate
                            .advanced(c.estimate.bearing_to(u.pos), range * 0.6);
                        Self::nav(cmds, u, goal);
                        cmds.push(Command::AssignTarget {
                            unit: u.id,
                            contact: c.id,
                        });
                        mind.phase = Phase::Intercept;
                        self.note(
                            u.id,
                            Some(c.id),
                            "cap_intercept",
                            format!("{}：在防区内拦截C{}，兼顾突击波掩护", u.name, c.id.0),
                            format!(
                                "{}: intercept C{} within patrol/escort limits",
                                u.name, c.id.0
                            ),
                        );
                    } else {
                        cmds.push(Command::ClearTarget { unit: u.id });
                        // Empty/low-fuel CAP return to a close orbit; the
                        // existing fuel rule performs individual RTB.
                        let station = if !armed || !rested {
                            home_pos
                        } else {
                            defended
                                .map(|a| a.pos.advanced(self.threat_axis_deg + 180.0, 8.0))
                                .unwrap_or_else(|| home_pos.advanced(self.threat_axis_deg, 22.0))
                        };
                        Self::orbit(cmds, u, station, 6.0);
                        mind.phase = Phase::Patrol;
                        self.note(
                            u.id,
                            None,
                            "cap_cover",
                            format!("{}：回到防空/护航阵位，停止远追", u.name),
                            format!("{}: resume CAP/escort station; break pursuit", u.name),
                        );
                    }
                    cmds.push(Command::SetAltitude {
                        unit: u.id,
                        altitude_m: 3000.0,
                    });
                }
                Role::Aew => {
                    let return_budget=u.pos.range_to(home_pos)/self.cruise_of(&u.class)*3600.0*1.3+600.0;
                    if u.endurance_s.unwrap_or(0.0)<=return_budget {
                        cmds.push(Command::ReturnToBase{unit:u.id});
                        self.note(u.id,None,"aew_handover",format!("{}：返航储备到达；接替仍须核对实际起飞回执",u.name),format!("{}: return reserve reached; confirm replacement from actual launch receipts",u.name));continue;
                    }
                    let danger = self.air_threat(obs, u.pos, 60.0).is_some()
                        || self.observed_missile_danger(obs,u.pos);
                    cmds.push(Command::SetAltitude {
                        unit: u.id,
                        altitude_m: if danger {150.0}else{3000.0},
                    });
                    // Patrol ON the threat axis, not astern: the AEW radar's
                    // 60nm surface reach is the fleet's only persistent ship
                    // track toward the opponent, and an astern orbit covers
                    // exactly the wrong half. The danger branch still drops
                    // the machine 40nm astern until the threat passes.
                    let desired = home_pos.advanced(
                        if danger { self.threat_axis_deg + 180.0 } else { self.threat_axis_deg },
                        if danger { 40.0 } else { 50.0 },
                    );
                    let station=self.db.as_ref().map(|db|self.supervision.support_station(obs,db,home,desired)).unwrap_or(desired);
                    Self::orbit(cmds, u, station, 6.0);
                    cmds.push(Command::ClearTarget { unit: u.id });
                    mind.phase = if danger {
                        Phase::Protect
                    } else {
                        Phase::Patrol
                    };
                    self.note(
                        u.id,
                        None,
                        if danger { "aew_evade" } else { "aew_station" },
                        format!(
                            "{}：{}，保持预警覆盖",
                            u.name,
                            if danger {
                                "向舰队后方避让敌机"
                            } else {
                                "威胁轴前出预警巡逻"
                            }
                        ),
                        format!(
                            "{}: {} with radar cover",
                            u.name,
                            if danger {
                                "evade astern"
                            } else {
                                "forward AEW patrol on the threat axis"
                            }
                        ),
                    );
                }
                Role::Scout => {
                    let return_budget=(u.pos.range_to(home_pos)/self.cruise_of(&u.class)*3600.0)*1.3+300.0;
                    if u.endurance_s.unwrap_or(10000.0)<self.params.cap_fuel_rtb_s.max(return_budget) {
                        cmds.push(Command::ReturnToBase {unit:u.id});continue;
                    }
                    let danger=self.air_threat(obs,u.pos,30.0);
                    let missile_danger=self.observed_missile_danger(obs,u.pos);
                    let evade=danger.is_some()||missile_danger;
                    let package_target=self.packages.values().find(|p|p.searcher==Some(u.id)).and_then(|p|p.target);
                    let ship=obs.contacts.iter().filter(|c|c.category==PlatformKind::Ship && !self.sensor_contact(c) && !c.bearing_only && c.bda_hp_fraction!=Some(0.0)&&package_target.is_none_or(|id|c.id==id))
                        .min_by(|a,b|u.pos.range_to(a.estimate).total_cmp(&u.pos.range_to(b.estimate)));
                    let reach=u.sensors.iter().filter_map(|s|s.vs_surface).fold(0.0,f64::max).clamp(35.0,120.0)*0.8;
                    let sweep=((obs.tick/240+u.id.0 as i64)%3) as f64-1.0;
                    let desired=if evade {home_pos.advanced(self.threat_axis_deg+180.0,20.0)}else if let Some(c)=ship {
                        let p=c.estimate.advanced(c.est_course_deg,c.est_speed_kts.max(0.0)*120.0/3600.0);
                        p.advanced(p.bearing_to(home_pos),reach)
                    }else{home_pos.advanced(self.threat_axis_deg,70.0).advanced(self.threat_axis_deg+90.0,sweep*10.0)};
                    let point=self.db.as_ref().map(|db|self.supervision.support_station(obs,db,home,desired)).unwrap_or(desired);
                    if obs.tick-mind.last_asw_retask>=120 || evade&&mind.phase!=Phase::Protect {
                        cmds.push(Command::RetaskAircraft {unit:u.id,mission:AirMission::Recon {point},manual:false});mind.last_asw_retask=obs.tick;
                    }
                    cmds.push(Command::SetAltitude {unit:u.id,altitude_m:if evade {150.0}else{3000.0}});cmds.push(Command::SetRadar {unit:u.id,active:!evade});cmds.push(Command::SetWeaponsFree {unit:u.id,free:false});
                    if let Some(c)=danger {cmds.push(Command::AssignTarget {unit:u.id,contact:c.id});}else{cmds.push(Command::ClearTarget {unit:u.id});}
                    mind.phase=if evade {Phase::Protect}else{Phase::Patrol};
                    self.note(u.id,ship.map(|c|c.id),if evade{"recon_evade"}else if ship.is_some(){"recon_shadow"}else{"recon_search"},
                        format!("{}：{}",u.name,if evade{"退向CAP保护区，保留侦察机"}else if ship.is_some(){"按观察保持海面航迹，支援突击接替"}else{"沿威胁轴分区搜索，保持返航储备"}),
                        format!("{}: {}",u.name,if evade{"withdraw toward CAP cover"}else if ship.is_some(){"shadow observed surface track / relay"}else{"search threat-axis sectors with return reserve"}));
                }
                Role::Ew => {
                    let cap=harpoon_core::rules::electronic_warfare::capability(&u.class).unwrap();
                    let threat=self.air_threat(obs,u.pos,35.0).is_some()||self.observed_missile_danger(obs,u.pos);
                    let binding=self.supervision.ew.get(&u.id);
                    let strike=obs.units.iter().filter(|a|self.role_for(a)==Role::Striker && a.mission.as_deref()!=Some("rtb")
                        && binding.is_some_and(|b|b.until>obs.tick&&b.members.contains(&a.id))).min_by(|a,b|u.pos.range_to(a.pos).total_cmp(&u.pos.range_to(b.pos)));
                    let station=strike.map(|a|a.pos.advanced(self.threat_axis_deg+180.0,12.0)).unwrap_or(home_pos);
                    Self::orbit(cmds,u,if threat {home_pos}else{station},5.0);
                    cmds.push(Command::SetAltitude {unit:u.id,altitude_m:if threat {150.0}else{3000.0}});
                    cmds.push(Command::SetWeaponsFree {unit:u.id,free:false});
                    let mut contacts:Vec<_>=obs.contacts.iter().filter(|c|!c.stale && c.sensors.iter().any(|s|self.db.as_ref().and_then(|db|db.sensors.get(s)).is_some_and(|s|s.kind==harpoon_core::db::SensorKind::Esm)))
                        .collect();
                    contacts.sort_by(|a,b|u.pos.range_to(a.estimate).total_cmp(&u.pos.range_to(b.estimate)));
                    let mode=if strike.is_none()||threat {"off"}else if !contacts.is_empty()&&cap.spot_targets>0 {"spot"}else if cap.standoff {"standoff"}else{"escort"};
                    cmds.push(Command::SetJamming {unit:u.id,mode:mode.into(),contacts:if mode=="spot" {contacts.into_iter().take(cap.spot_targets).map(|c|c.id).collect()}else{vec![]}});
                    mind.phase=if threat {Phase::Protect}else{Phase::Patrol};
                    self.note(u.id,None,if threat {"ew_evade"}else if strike.is_none(){"ew_reserve"}else{"ew_support"},format!("{}：{}电子战支援；干扰效果取决于已核验雷达与几何",u.name,mode),format!("{}: {} electronic support; effect depends on verified radar and geometry",u.name,mode));
                }
                Role::AswScreen => {
                    let task=self.supervision.asw.get(&u.id);
                    let nominal=task.map(|t|t.area).unwrap_or_else(||self.safe_asw_area(obs, home));
                    let sector=((obs.tick/240+u.id.0 as i64)%4) as f64;
                    let area=if task.is_some()||self.best_sub_target(obs,home).is_some(){nominal}else{nominal.advanced(self.threat_axis_deg+90.0, (sector-1.5)*3.0)};
                    let buoy_overlap = obs
                        .units
                        .iter()
                        .filter(|a| {
                            a.class == harpoon_core::rules::asw::SONOBUOY_CLASS
                                && a.pos.range_to(area) < 10.0
                        })
                        .count();
                    let no_search_sector=self.db.as_ref().is_some_and(|db|super::assessment::asw_station(obs,db,home,0).is_none());
                    if (buoy_overlap >= 8 || no_search_sector) && task.is_none_or(|t|t.contact.is_none()) {
                        cmds.push(Command::ReturnToBase { unit: u.id });
                        self.note(u.id,None,"asw_reserve",format!("{}：附近已有浮标，停止重复投放并返航待命",u.name),format!("{}: buoy overlap reached; RTB to reserve instead of duplicating drops",u.name));
                        continue;
                    }
                    let danger = self.air_threat(obs, u.pos, 40.0).is_some()
                        || self.surface_danger(obs, u.pos, 45.0) || self.observed_missile_danger(obs,u.pos);
                    if obs.tick-mind.last_asw_retask>=240 || danger && mind.phase!=Phase::Protect {
                        cmds.push(Command::RetaskAircraft {unit:u.id,mission:AirMission::Asw {area:if danger {home_pos}else{area},radius_nm:1.0},manual:false});
                        mind.last_asw_retask=obs.tick;
                    }
                    mind.phase=if danger {Phase::Protect}else{Phase::Patrol};
                    if let Some((cid, d)) = self.best_sub_target(obs, u).filter(|(_, d)| *d < 6.0) {
                        if let Some((weapon,_,_))=self.pick_weapon(u,&[WeaponKind::Lwt,WeaponKind::AswRocket],d) {
                            if obs.contacts.iter().find(|c|c.id==cid).is_some_and(|c|c.own_in_flight<2) {cmds.push(Command::Fire {unit:u.id,contact:cid,weapon,rounds:1});}
                        }
                    } else {
                        cmds.push(Command::ClearTarget { unit: u.id });
                    }
                    self.note(
                        u.id,
                        None,
                        if danger { "asw_evade" } else { "asw_screen" },
                        format!(
                            "{}：{}",
                            u.name,
                            if danger {
                                "避开敌机，退入舰队防空圈"
                            } else {
                                "在近程屏障搜索潜艇"
                            }
                        ),
                        format!(
                            "{}: {}",
                            u.name,
                            if danger {
                                "withdraw into fleet air cover"
                            } else {
                                "search protected ASW barrier"
                            }
                        ),
                    );
                }
                _ => {}
            }
            self.air_minds.insert(u.id, mind);
        }
    }

    fn target_value(&self, c: &ContactObs, u: &UnitObs) -> f64 {
        let name = c.class_name.as_deref().unwrap_or("").to_ascii_lowercase();
        let value = if name.contains("cv")
            || name.contains("carrier")
            || name.contains("ford")
            || name.contains("nimitz")
        {
            60.0
        } else {
            30.0
        };
        value - u.pos.range_to(c.estimate) * 0.08 - c.own_in_flight as f64 * 2.0
            - self.supervision.attacks.get(&c.id).map(|a|a.unconfirmed_batches as f64*value).unwrap_or(0.0)
    }

    fn safe_asw_area(&self, obs: &Observation, home: &UnitObs) -> Vec2 {
        let nominal = home
            .pos
            .advanced(self.threat_axis_deg, self.params.asw_barrier_nm);
        self.best_sub_target(obs, home)
            .filter(|(cid, d)| {
                *d < 25.0
                    && self
                        .air_threat(obs, contact_pos(obs, *cid, home.pos), 45.0)
                        .is_none()
            })
            .map(|(cid, _)| contact_pos(obs, cid, nominal))
            .unwrap_or(nominal)
    }

    /// Resolve observed identity once per launch package. A changed target is
    /// an explicit new assignment, never a claim that two uncertain tracks are
    /// the same hidden entity.
    fn refresh_strike_packages(&mut self, obs: &Observation) {
        let mut groups: BTreeMap<(EntityId,i64),Vec<&UnitObs>> = BTreeMap::new();
        for u in obs.units.iter().filter(|u|u.mission.as_deref()==Some("strike")) {
            if let Some(m)=self.air_minds.get(&u.id) {
                if self.loaded_range(u,WeaponKind::Ascm)>0.0 {groups.entry((m.home,m.born)).or_default().push(u);}
            }
        }
        self.packages.retain(|_,p|p.members.iter().any(|id|obs.units.iter().any(|u|u.id==*id&&u.mission.as_deref()==Some("strike"))));
        for ((home,born),mut members) in groups {
            members.sort_by_key(|u|u.id);
            let lead=members[0]; let mut reference=self.air_minds[&lead.id].clone();
            let old=reference.target;
            let resolved=self.strike_track(obs,&mut reference);
            let fresh=resolved.as_ref().is_some_and(|c|!c.stale && !c.bearing_only && c.quality>=self.params.strike_min_q);
            let last_good=old.and_then(|id|self.track_memory.get(&id)
                .filter(|(_,c)|!c.bearing_only && c.quality>=self.params.strike_min_q).map(|(t,_)|*t)).unwrap_or(born);
            let mut chosen=resolved;
            let mut retargeted=false;
            let confirmed_gone=old.is_some_and(|id|obs.contacts.iter().any(|c|c.id==id&&c.bda_hp_fraction==Some(0.0)));
            if !fresh && (confirmed_gone || obs.tick-last_good>=self.params.strike_retarget_s) {
                let alternative=obs.contacts.iter().filter(|c|c.category==PlatformKind::Ship && !c.stale
                    && !c.bearing_only && c.quality>=self.params.strike_min_q && !self.sensor_contact(c)
                    && c.bda_hp_fraction!=Some(0.0))
                    .filter(|c|self.supervision.target_allowed(c.id,obs.tick))
                    .filter(|c|members.iter().any(|u| {
                        let transit=(u.pos.range_to(c.estimate)-self.loaded_range(u,WeaponKind::Ascm)*0.85).max(0.0)/self.cruise_of(&u.class)*3600.0;
                        let back=self.home(obs,u).map(|h|c.estimate.range_to(h.pos)/self.cruise_of(&u.class)*3600.0).unwrap_or(0.0);
                        u.endurance_s.unwrap_or(0.0)>transit+back*1.3+300.0
                    }))
                    .min_by(|a,b|lead.pos.range_to(a.estimate).total_cmp(&lead.pos.range_to(b.estimate)));
                if let Some(c)=alternative {chosen=Some(c.clone());retargeted=old!=Some(c.id);}
            }
            let package_id=self.packages.iter().find(|(_,p)|p.home==home&&p.born==born).map(|(id,_)|*id).unwrap_or(lead.id);
            let mut package=self.packages.remove(&package_id).unwrap_or(Package{home,born,members:vec![],target:old,searcher:None,stage:"assemble".into(),changed_at:obs.tick});
            package.members=members.iter().map(|u|u.id).collect();
            if let Some(target)=chosen {
                if package.target!=Some(target.id){package.changed_at=obs.tick;}
                package.target=Some(target.id);
                for u in &members {
                    if let Some(m)=self.air_minds.get_mut(&u.id) {
                        if m.target!=Some(target.id) {m.target=Some(target.id);m.attack_since=None;}
                        if retargeted && m.phase!=Phase::Assemble {m.phase=Phase::Attack;}
                    }
                }
                if retargeted {self.note(lead.id,Some(target.id),"strike_retarget",
                    format!("机群原目标航迹失效，明确改派本方新鲜接触C{}；不认定为同一目标",target.id.0),
                    format!("package reassigns to fresh observed C{}; no hidden identity inferred",target.id.0));}
                let fresh=!target.stale&&!target.bearing_only&&target.quality>=self.params.strike_min_q;
                let old_searcher=package.searcher;
                if package.searcher.is_some_and(|id|!obs.units.iter().any(|u|u.id==id&&u.mission.as_deref()!=Some("rtb")&&self.air_threat(obs,u.pos,45.0).is_none()&&!self.observed_missile_danger(obs,u.pos))){package.searcher=None;}
                if (!fresh||old_searcher.is_some())&&package.searcher.is_none(){
                    package.searcher=obs.units.iter().filter(|u|self.role_for(u)==Role::Scout&&u.mission.as_deref()!=Some("rtb")&&u.pos.range_to(lead.pos)<80.0&&u.endurance_s.unwrap_or(0.0)>1200.0
                        &&self.relay_covers(u,&target)&&self.air_threat(obs,u.pos,45.0).is_none()&&!self.observed_missile_danger(obs,u.pos))
                        .min_by(|a,b|a.pos.range_to(target.estimate).total_cmp(&b.pos.range_to(target.estimate))).map(|u|u.id)
                        .or_else(||members.iter().filter(|u|self.air_threat(obs,u.pos,45.0).is_none()&&!self.observed_missile_danger(obs,u.pos)).map(|u|u.id).min());
                }
                if old_searcher!=package.searcher {self.note(lead.id,Some(target.id),"package_relay",format!("机群搜索接替指定{:?}；该来源持续工作，其他成员保持低空",package.searcher.map(|id|id.0)),format!("Package relay {:?}; keep source working while other members remain low",package.searcher.map(|id|id.0)));}
                package.stage=if !fresh {"search"}else if obs.tick-born<self.params.strike_assembly_s {"assemble"}else if members.iter().filter(|u|Some(u.id)!=package.searcher).all(|u|u.pos.range_to(target.estimate)<=self.loaded_range(u,WeaponKind::Ascm)*0.95){"release"}else{"approach"}.into();
            }
            self.packages.insert(package_id,package);
        }
    }

    fn fly_strike(
        &mut self,
        obs: &Observation,
        u: &UnitObs,
        mind: &mut AirMind,
        cmds: &mut Vec<Command>,
    ) {
        // VLow = 21–60m (2026 p4-3); 50m is a policy choice. Never order
        // SSkim (0–20m) or dogfight from this flight profile (§4.5.2/4.5.4).
        let terminal_altitude=self.params.strike_altitude_m.clamp(21.0,60.0);
        let observed_target=mind.target.and_then(|id|obs.contacts.iter().find(|c|c.id==id));
        let fresh_target=observed_target.is_some_and(|c|!c.stale&&!c.bearing_only&&c.quality>=self.params.strike_min_q);
        let designated_relay=self.packages.values().any(|p|p.home==mind.home&&p.born==mind.born&&p.searcher==Some(u.id));
        let cover=observed_target.is_some_and(|target|obs.units.iter().any(|a|matches!(self.role_for(a),Role::Aew|Role::Scout)&&a.mission.as_deref()!=Some("rtb")
            &&self.relay_covers(a,target)));
        cmds.push(Command::SetRadar {unit:u.id,active:designated_relay||!fresh_target||!cover});
        let far=mind.target.and_then(|id|obs.contacts.iter().find(|c|c.id==id)).is_some_and(|c|u.pos.range_to(c.estimate)>120.0_f64.max(self.loaded_range(u,WeaponKind::Ascm)*1.4));
        let has_relay=self.packages.values().any(|p|p.home==mind.home&&p.born==mind.born&&p.searcher.is_some_and(|id|id!=u.id));
        let altitude=if far && !has_relay && matches!(mind.phase,Phase::Assemble|Phase::Ingress) {3000.0}else{terminal_altitude};
        cmds.push(Command::SetAltitude {
            unit: u.id,
            altitude_m: altitude,
        });
        if harpoon_core::rules::flight_profiles::supported(&u.class) {cmds.push(Command::SetThrottle {unit:u.id,setting:"cruise".into()});}
        // Hold anti-ship automatic selection during ingress. Self-defense
        // uses only an air contact; manual anti-ship salvos stay coordinated.
        cmds.push(Command::SetWeaponsFree {
            unit: u.id,
            free: false,
        });
        if let Some(c) = self.air_threat(obs, u.pos, 25.0) {
            cmds.push(Command::AssignTarget {
                unit: u.id,
                contact: c.id,
            });
        } else {
            cmds.push(Command::ClearTarget { unit: u.id });
        }
        let target = self.strike_track(obs, mind);
        let Some(target) = target.as_ref() else {
            cmds.push(Command::ReturnToBase { unit: u.id });
            self.note(
                u.id,
                mind.target,
                "strike_track_lost",
                format!("{}：目标记录消失或已识别为传感器，取消突击返航", u.name),
                format!(
                    "{}: target record gone or identified as sensor; abort and RTB",
                    u.name
                ),
            );
            return;
        };
        let range = self.loaded_range(u, WeaponKind::Ascm);
        let return_seconds = self.home(obs,u).map(|h|u.pos.range_to(h.pos)/self.cruise_of(&u.class).max(60.0)*3600.0).unwrap_or(0.0);
        if range <= 0.0 || u.endurance_s.unwrap_or(10000.0) < self.params.cap_fuel_rtb_s.max(return_seconds*1.3+300.0) {
            cmds.push(Command::ReturnToBase { unit: u.id });
            return;
        }
        let cohort: Vec<_> = obs
            .units
            .iter()
            .filter(|a| {
                self.air_minds.get(&a.id).is_some_and(|m| {
                    m.home == mind.home
                        && (m.born - mind.born).abs() < 60
                        && m.target == mind.target
                }) && a.mission.as_deref() == Some("strike")
                    && self.loaded_range(a, WeaponKind::Ascm) > 0.0
            })
            .collect();
        let ready = cohort
            .iter()
            .all(|a| a.altitude_m <= altitude+60.0 && a.pos.range_to(mind.assembly) < 20.0);
        // Standoff fits (>=200nm) must not burn a full assembly window and a
        // descent to wave-top inside the enemy engagement envelope: release
        // geometry works from cruise altitude.
        let assembly_wait = if range>=200.0 {60} else {self.params.strike_assembly_s};
        if mind.phase == Phase::Assemble {
            if obs.tick - mind.born >= assembly_wait && (ready || obs.tick-mind.born>=assembly_wait+180) {
                mind.phase = Phase::Ingress;
            } else {
                Self::orbit(cmds, u, mind.assembly, 4.0);
                self.note(
                    u.id,
                    Some(target.id),
                    "strike_assemble",
                    format!("{}：突击波集结，等待全组下降到{}米", u.name, altitude),
                    format!(
                        "{}: assemble wave; wait for descent to {}m",
                        u.name, altitude
                    ),
                );
                return;
            }
        }
        let package=self.packages.values().find(|p|p.home==mind.home&&p.born==mind.born);
        let package_relay=package.map(|p|p.searcher==Some(u.id)).unwrap_or_else(||cohort.iter().map(|a|a.id).chain(std::iter::once(u.id)).min()==Some(u.id))
            && self.air_threat(obs,u.pos,45.0).is_none()&&!self.observed_missile_danger(obs,u.pos);
        if target.stale || target.bearing_only || target.quality < self.params.strike_min_q {
            if obs.tick - mind.born > 1800 {
                cmds.push(Command::ReturnToBase { unit: u.id });
                self.note(
                    u.id,
                    Some(target.id),
                    "strike_search_timeout",
                    format!("{}：搜索时限到达，保留弹药返航", u.name),
                    format!("{}: search time budget exhausted; RTB with weapons", u.name),
                );
                return;
            }
            // Navigate to an OBSERVED last-known estimate, never an enemy
            // entity. Search within this aircraft's own sensor reach; weapons
            // remain held until the side reacquires a fresh firing track.
            // One element trades concealment for radar horizon. The rest of
            // the package stays low; no hidden track or guaranteed fix is added.
            let search_lead = package_relay;
            if search_lead {mind.relay_until.get_or_insert(obs.tick+self.params.strike_relay_hold_s);}
            let search_altitude = if search_lead {3000.0}else{terminal_altitude};
            cmds.push(Command::SetAltitude {unit:u.id,altitude_m:search_altitude});
            let search_range = u
                .sensors
                .iter()
                .filter_map(|s| s.vs_surface)
                .fold(0.0, f64::max);
            let standoff = (search_range * 0.75).clamp(8.0, 40.0);
            let search = target
                .estimate
                .advanced(target.estimate.bearing_to(u.pos), standoff);
            Self::orbit(cmds, u, search, 3.0);
            mind.phase = Phase::Reacquire;
            self.note(
                u.id,
                Some(target.id),
                "strike_reacquire",
                format!(
                    "{}：{}米接近C{}最后位置搜索，重获有效航迹前禁射",
                    u.name, search_altitude, target.id.0
                ),
                format!(
                    "{}: {}m search near C{} last-known position; hold fire until reacquired",
                    u.name, search_altitude, target.id.0
                ),
            );
            return;
        }
        // Keep the searching element aloft while the other members descend.
        // Reversing its altitude immediately on reacquisition loses the very
        // track that made the package ready. Do not turn this into hidden truth.
        if package_relay && (package.is_some()||mind.relay_until.is_some_and(|until|obs.tick<until))
            && cohort.iter().any(|a|a.id!=u.id) {
            cmds.push(Command::SetAltitude {unit:u.id,altitude_m:3000.0});
            let point=target.estimate.advanced(target.estimate.bearing_to(u.pos),self.loaded_range(u,WeaponKind::Ascm)*0.8);
            Self::orbit(cmds,u,point,3.0);
            self.note(u.id,Some(target.id),"strike_relay",
                format!("{}：保持机群搜索接替，其他成员准备投放；本机暂不放弹",u.name),
                format!("{}: retain observed-track relay while other members prepare release",u.name));
            return;
        }
        if mind.phase == Phase::Reacquire {
            mind.phase = Phase::Attack;
        }
        // Recompute the flank against this side's current track, never a truth
        // entity. Do not keep chasing a release point behind a withdrawing ship.
        let predicted = intercept_estimate(u.pos, self.cruise_of(&u.class), target, range*0.85);
        let axis=self.home(obs,u).map(|h|h.pos.bearing_to(predicted)).unwrap_or(self.threat_axis_deg);
        let sign=if mind.home.0%2==0 {1.0}else{-1.0};
        mind.flank=predicted.advanced(axis+180.0+sign*25.0,range.max(35.0)*1.05);
        if mind.phase == Phase::Ingress && (u.pos.range_to(mind.flank) < 8.0 || u.pos.range_to(target.estimate)<range*0.95 || obs.tick-mind.born>self.params.strike_assembly_s+480) {
            mind.phase = Phase::Attack;
        }
        if mind.phase == Phase::Ingress {
            Self::nav(cmds, u, mind.flank);
            self.note(
                u.id,
                Some(target.id),
                "strike_ingress",
                format!(
                    "{}：{}米{}，目标C{}",
                    u.name, altitude, if altitude<=60.0 {"超低空侧翼突防"}else{"侧翼转场，接近后下降"}, target.id.0
                ),
                format!(
                    "{}: {}m {} on C{}",
                    u.name, altitude, if altitude<=60.0 {"VLow flanking ingress"}else{"flanking transit; descend near target"}, target.id.0
                ),
            );
        } else {
            let release = predicted.advanced(predicted.bearing_to(u.pos), range * 0.85);
            Self::nav(cmds, u, release);
            let release_floor=u.weapons.iter().filter(|w|w.operational&&w.ammo>0&&self.kind_of(&w.weapon)==Some(WeaponKind::Ascm)).map(|w|w.min_launch_altitude_m).fold(0.0_f64,f64::max);
            // Standoff releases happen from cruise altitude — LRASM-class
            // weapons have no ceiling in the engine, and forcing a descent to
            // wave-top just parks the flight inside the SAM envelope.
            let release_altitude=if range>=200.0 {u.altitude_m.max(release_floor+20.0)} else if release_floor>0.0{release_floor+20.0}else{terminal_altitude};
            cmds.push(Command::SetAltitude{unit:u.id,altitude_m:release_altitude});
            let since=*mind.attack_since.get_or_insert(obs.tick);
            let all_in_range = cohort.iter().all(|a| {
                a.pos.range_to(target.estimate) <= self.loaded_range(a, WeaponKind::Ascm) * 0.95
            });
            // A straggler has a finite rendezvous budget. Once it expires,
            // release a valid in-range element; never extend weapon range.
            let regroup_expired=obs.tick-since>=180;
            if (all_in_range || regroup_expired) && u.pos.range_to(target.estimate)<=range*0.95 && u.altitude_m>=release_floor && u.altitude_m <= release_altitude+60.0 {
                for w in &u.weapons {
                    if w.operational && w.ammo > 0
                        && u.altitude_m>=w.min_launch_altitude_m
                        && w.cooldown_s <= 0.0
                        && self.kind_of(&w.weapon) == Some(WeaponKind::Ascm)
                        && u.pos.range_to(target.estimate) <= self.range_of(&w.weapon) * 0.95
                    {
                        cmds.push(Command::Fire {
                            unit: u.id,
                            contact: target.id,
                            weapon: w.weapon.clone(),
                            rounds: w.ammo.min(4) as u8,
                        });
                    }
                }
                self.note(
                    u.id,
                    Some(target.id),
                    "strike_salvo",
                    format!(
                        "{}：同组进入射程，向C{}下令齐射；随后返航",
                        u.name, target.id.0
                    ),
                    format!(
                        "{}: wave in range; order salvo at C{}, then RTB",
                        u.name, target.id.0
                    ),
                );
            } else {
                self.note(
                    u.id,
                    Some(target.id),
                    "strike_attack",
                    format!("{}：等待释放条件，距目标{:.1}/{:.1}海里、高度{:.0}米、同组就绪{}", u.name,u.pos.range_to(target.estimate),range*0.95,u.altitude_m,all_in_range),
                    format!("{}: release held; distance {:.1}/{:.1}nm, altitude {:.0}m, group ready {}", u.name,u.pos.range_to(target.estimate),range*0.95,u.altitude_m,all_in_range),
                );
            }
        }
    }

    /// Commander planning budget: round trip with a return reserve. This is
    /// not a weapon range or a replacement for the core endurance calculation.
    fn strike_sortie_reach(&self,class:&str)->f64 {
        self.db.as_ref().and_then(|d|d.platforms.get(class))
            .map(|p|p.cruise_speed_kts*p.endurance_s/3600.0/2.6).unwrap_or(0.0)
    }

    fn strike_track(&self, obs:&Observation, mind:&mut AirMind) -> Option<ContactObs> {
        let cid=mind.target?;
        if let Some(c)=obs.contacts.iter().find(|c|c.id==cid && !c.stale && !c.bearing_only && c.quality>=self.params.strike_min_q && !self.sensor_contact(c) && c.bda_hp_fraction!=Some(0.0)) {return Some(c.clone());}
        let (tick,old)=self.track_memory.get(&cid)?;
        let age=obs.tick-*tick;
        if age>600 || old.bda_hp_fraction==Some(0.0) {return None;}
        let predicted=old.estimate.advanced(old.est_course_deg,old.est_speed_kts.max(0.0)*age as f64/3600.0);
        let mut candidates:Vec<_>=obs.contacts.iter().filter(|c|c.category==old.category && !c.stale && !c.bearing_only && !self.sensor_contact(c)
            && c.quality>=self.params.strike_min_q && c.bda_hp_fraction!=Some(0.0)
            && match (&old.class_name,&c.class_name) {(Some(a),Some(b))=>a==b,_=>true})
            .map(|c|(predicted.range_to(c.estimate),c)).filter(|(d,_)|*d<8.0+age as f64/120.0).collect();
        candidates.sort_by(|a,b|a.0.total_cmp(&b.0));
        if let Some((d,c))=candidates.first() {
            if candidates.get(1).is_none_or(|(other,_)|other-d>2.0) {mind.target=Some(c.id);return Some((*c).clone());}
        }
        let mut search=old.clone();search.estimate=predicted;search.stale=true;
        Some(search)
    }
}

/// Fixed-point pursuit solution using only the contact's observed velocity.
fn intercept_estimate(from:Vec2, speed_kts:f64, c:&ContactObs, release_range:f64)->Vec2 {
    let mut p=c.estimate;
    for _ in 0..6 {
        let seconds=((from.range_to(p)-release_range).max(0.0)/speed_kts.max(60.0)*3600.0).min(900.0);
        p=c.estimate.advanced(c.est_course_deg,c.est_speed_kts.clamp(0.0,80.0)*seconds/3600.0);
    }
    p
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn a_budget_rejected_launch_does_not_consume_the_hosts_launch_clock(){
        let db=Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
        let sc=harpoon_core::scenario_from_json(r#"{"id":"rejected-launch","name":"test","duration_s":1000,"seed":1,"weather":{"visibility_nm":1,"jam":[0,0]},"context":{"air_wings":{"A":[{"class":"cn_j15","count":12}],"B":[{"class":"cn_j15","count":12}]}},"units":[{"side":"Blue","class":"cn_001","name":"A","pos":[0,0]},{"side":"Blue","class":"cn_002","name":"B","pos":[100,0]}]}"#).unwrap();
        let mut w=harpoon_core::load_scenario(sc,db.clone()).unwrap();harpoon_core::air::launch_aircraft(&mut w,EntityId(1),"cn_j15",8,&AirMission::Cap{station:Vec2::zero(),radius_nm:10.0,follow:true}).unwrap();
        let mut staff=TacticalStaff::with_db(Side::Blue,db);let o=harpoon_core::observe(&w,Side::Blue,0,256);let commands=staff.decide(&o);
        assert!(!commands.iter().any(|c|matches!(c,Command::LaunchAircraft{unit,..}if *unit==EntityId(2))));assert!(!staff.last_launch.contains_key(&EntityId(2)));
    }
    fn track(id:u32,x:f64)->ContactObs {serde_json::from_value(serde_json::json!({"id":id,"category":"Ship","class_name":"Known ship","estimate":{"x":x,"y":0},"est_course_deg":90,"est_speed_kts":30,"quality":0.8,"bearing_only":false,"last_seen_s":0,"stale":false,"sensors":["radar_air_long"]})).unwrap()}
    fn obs()->Observation {
        let sc=harpoon_core::scenario_from_json(r#"{"id":"test","name":"test","duration_s":1000,"seed":7,"weather":{"visibility_nm":1,"jam":[0,0]},"units":[{"side":"Blue","class":"ffg","pos":[0,0]}]}"#).unwrap();
        harpoon_core::observe(&harpoon_core::load_scenario(sc,Database::built_in()).unwrap(),Side::Blue,0,0)
    }
    #[test]fn withdrawing_track_produces_a_forward_intercept(){
        let c=track(1,100.0);let p=intercept_estimate(Vec2::zero(),450.0,&c,40.0);assert!(p.x>100.0 && p.x<108.0);assert!(p.y.abs()<1e-6);
    }
    #[test]fn reassociation_requires_one_clear_candidate_and_preserves_uncertainty(){
        let mut o=obs();o.tick=120;o.contacts=vec![track(2,101.0),track(3,102.0)];
        let mut staff=TacticalStaff::new(Side::Blue);staff.track_memory.insert(ContactId(1),(0,track(1,100.0)));
        let mut mind=AirMind {home:EntityId(1),born:0,phase:Phase::Reacquire,target:Some(ContactId(1)),assembly:Vec2::zero(),flank:Vec2::zero(),attack_since:None,last_asw_retask:0,relay_until:None};
        assert!(staff.strike_track(&o,&mut mind).unwrap().stale);assert_eq!(mind.target,Some(ContactId(1)));
        o.contacts.pop();assert!(!staff.strike_track(&o,&mut mind).unwrap().stale);assert_eq!(mind.target,Some(ContactId(2)));
    }
    #[test]fn relaunched_z18j_is_aew_even_without_a_named_aew_callsign(){
        let mut o=obs();o.units[0].kind=PlatformKind::Aircraft;o.units[0].class="cn_z18j".into();o.units[0].name="BLUE Z-18J".into();
        assert_eq!(TacticalStaff::new(Side::Blue).role_for(&o.units[0]),Role::Aew);
    }
    #[test]fn a_stale_record_still_in_the_list_does_not_hide_its_unique_fresh_replacement(){
        let mut o=obs();o.tick=120;
        let original=track(1,100.0);let mut stale=original.clone();stale.stale=true;stale.quality=0.0;
        o.contacts=vec![stale,track(2,101.0)];
        let mut staff=TacticalStaff::new(Side::Blue);staff.track_memory.insert(ContactId(1),(0,original));
        let mut mind=AirMind {home:EntityId(1),born:0,phase:Phase::Reacquire,target:Some(ContactId(1)),assembly:Vec2::zero(),flank:Vec2::zero(),attack_since:None,last_asw_retask:0,relay_until:None};
        let result=staff.strike_track(&o,&mut mind).unwrap();
        assert_eq!(result.id,ContactId(2));assert!(!result.stale);
    }

    fn searching_package() -> (TacticalStaff,Observation,Vec<EntityId>) {
        let db=Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
        let sc=harpoon_core::scenario_from_json(r#"{"id":"relay-test","name":"Relay test","duration_s":2000,"seed":7,
            "weather":{"visibility_nm":1,"jam":[0,0]},"context":{"air_wings":{"CV":[{"class":"us_f18e_asm","count":2}]}},
            "units":[{"side":"Blue","class":"us_ford","name":"CV","pos":[0,0]}]}"#).unwrap();
        let mut world=harpoon_core::load_scenario(sc,db.clone()).unwrap();
        let cv=world.entities.values().next().unwrap().id;
        harpoon_core::air::launch_planned_aircraft(&mut world,cv,"us_f18e_asm",2,
            &AirMission::Strike {contact:ContactId(1)},Some(&harpoon_core::commands::LaunchProfile {waypoints:Vec::new(),altitude_m:50.0,weapons_free:false,manual_strike:true,engagement:None})).unwrap();
        let ids:Vec<_>=world.entities.values().filter(|u|u.kind==PlatformKind::Aircraft).map(|u|u.id).collect();
        let mut o=harpoon_core::observe(&world,Side::Blue,0,0);o.contacts=vec![track(1,100.0)];
        let mut staff=TacticalStaff::with_db(Side::Blue,db);
        staff.decide(&o);staff.take_notes();
        for id in &ids {let m=staff.air_minds.get_mut(id).unwrap();m.phase=Phase::Reacquire;m.born=0;}
        for u in &mut o.units {if ids.contains(&u.id) {u.pos=Vec2::new(80.0,0.0);u.altitude_m=50.0;}}
        o.tick=400;o.contacts[0].stale=true;o.contacts[0].quality=0.0;
        (staff,o,ids)
    }

    #[test]fn relay_stays_up_for_other_members_and_survives_a_save_roundtrip(){
        let (mut staff,mut o,ids)=searching_package();
        let mut distant_aew=o.units.iter().find(|u|u.kind==PlatformKind::Aircraft).unwrap().clone();distant_aew.id=EntityId(999);distant_aew.class="us_e2d".into();distant_aew.mission=Some("recon".into());distant_aew.pos=Vec2::zero();for sensor in &mut distant_aew.sensors{sensor.active=true;sensor.vs_surface=Some(5.0);}o.units.push(distant_aew);
        staff.decide(&o);staff.take_notes();
        let db=staff.db.clone().unwrap();
        let mut restored:TacticalStaff=serde_json::from_slice(&serde_json::to_vec(&staff).unwrap()).unwrap();restored.restore_database(db);
        o.tick=430;o.contacts[0].stale=false;o.contacts[0].quality=0.8;
        o.units.iter_mut().find(|u|u.id==ids[0]).unwrap().altitude_m=3000.0;
        o.units.iter_mut().find(|u|u.id==ids[1]).unwrap().altitude_m=81.0;
        let commands=staff.decide(&o);
        assert_eq!(serde_json::to_value(&commands).unwrap(),serde_json::to_value(restored.decide(&o)).unwrap());
        let last_alt=commands.iter().filter_map(|c|match c {Command::SetAltitude {unit,altitude_m} if *unit==ids[0]=>Some(*altitude_m),_=>None}).last();
        assert_eq!(last_alt,Some(3000.0));
        assert_eq!(commands.iter().filter_map(|c|match c{Command::SetRadar{unit,active}if *unit==ids[0]=>Some(*active),_=>None}).last(),Some(true),"the designated relay must keep its radar on; existence of a distant AEW is not target coverage");
        let pending=serde_json::to_value(&staff).unwrap();
        assert!(pending["pending_salvos"].as_array().unwrap().iter().any(|p|p["unit"]==ids[1].0)||commands.iter().any(|c|matches!(c,Command::Fire{unit,..}if *unit==ids[1])));
        assert!(!pending["pending_salvos"].as_array().unwrap().iter().any(|p|p["unit"]==ids[0].0));
        o.tick=800;
        let later=staff.decide(&o);
        assert_eq!(later.iter().filter_map(|c|match c{Command::SetAltitude{unit,altitude_m}if *unit==ids[0]=>Some(*altitude_m),_=>None}).last(),Some(3000.0),"relay must not descend just because its old timer elapsed while other members remain armed");
        // The relay does not wait forever for members that have emptied their load.
        o.tick=830;
        for u in o.units.iter_mut().filter(|u|u.id==ids[1]) {for w in &mut u.weapons {w.ammo=0;}}
        let commands=staff.decide(&o);
        let last_alt=commands.iter().filter_map(|c|match c {Command::SetAltitude {unit,altitude_m} if *unit==ids[0]=>Some(*altitude_m),_=>None}).last();
        assert_eq!(last_alt,Some(81.0),"the last relay must climb to its legal ordnance-release band");
        o.tick=860;o.units.iter_mut().find(|u|u.id==ids[0]).unwrap().altitude_m=81.0;
        let commands=staff.decide(&o);assert!(commands.iter().any(|c|matches!(c,Command::Fire{unit,..}if *unit==ids[0])),"last relay with a current firing solution must not enter a fictitious coordination delay");
    }

    #[test]fn a_confirmed_destroyed_assignment_is_replaced_before_task_cancellation(){
        let (mut staff,mut o,ids)=searching_package();
        o.tick=430;o.contacts[0].stale=false;o.contacts[0].quality=0.8;
        o.contacts[0].bda_hp_fraction=Some(0.0);
        let mut alternative=track(2,120.0);alternative.class_name=Some("Different observed escort".into());
        o.contacts.push(alternative);
        let commands=staff.decide(&o);
        assert!(ids.iter().all(|id|staff.air_minds[id].target==Some(ContactId(2))));
        assert!(!commands.iter().any(|c|matches!(c,Command::ReturnToBase{unit} if ids.contains(unit))));
        assert!(!serde_json::to_value(&staff).unwrap()["pending_salvos"].as_array().unwrap().iter().any(|p|p["contact"]==1));
        assert!(staff.take_notes().iter().any(|n|n.action=="strike_retarget"));
        // An unavailable alternative must not become a fabricated firing fix.
        o.tick=460;o.contacts[1].bda_hp_fraction=Some(0.0);
        let commands=staff.decide(&o);
        assert!(!commands.iter().any(|c|matches!(c,Command::Fire{unit,..} if ids.contains(unit))));
    }

    #[test]fn covered_asw_sectors_do_not_trigger_relaunch_then_immediate_recovery(){
        let db=Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
        let sc=harpoon_core::scenario_from_json(r#"{"id":"covered-asw","name":"fixture","duration_s":1000,"seed":7,"weather":{"visibility_nm":1,"jam":[0,0]},"context":{"air_wings":{"CV":[{"class":"us_mh60r","count":4}]}},"units":[{"side":"Blue","class":"us_ford","name":"CV","pos":[0,0]}]}"#).unwrap();
        let w=harpoon_core::load_scenario(sc,db.clone()).unwrap();let mut o=harpoon_core::observe(&w,Side::Blue,0,0);
        let home=o.units[0].clone();
        for i in 0..2 {let mut cap=home.clone();cap.id=EntityId(100+i);cap.kind=PlatformKind::Aircraft;cap.class="us_f18e".into();cap.mission=Some("cap".into());cap.deck=None;o.units.push(cap);}
        for sector in 0..4 {for n in 0..8 {let mut buoy=home.clone();buoy.id=EntityId(200+sector*8+n);buoy.class=harpoon_core::rules::asw::SONOBUOY_CLASS.into();buoy.deck=None;buoy.pos=home.pos.advanced(home.course_deg+sector as f64*90.0,12.0);o.units.push(buoy);}}
        assert!(super::super::assessment::asw_station(&o,&db,&home,0).is_none());
        let mut staff=TacticalStaff::with_db(Side::Blue,db.clone());
        assert!(!staff.decide(&o).iter().any(|c|matches!(c,Command::LaunchAircraft{mission:AirMission::Asw{..},..})));
        o.units.retain(|u|u.id.0<224||u.id.0>=232);o.tick=30;
        let orders=staff.decide(&o);let area=orders.iter().find_map(|c|match c{Command::LaunchAircraft{mission:AirMission::Asw{area,..},..}=>Some(*area),_=>None}).expect("a newly uncovered sector can be searched");
        assert!(o.units.iter().filter(|u|u.class==harpoon_core::rules::asw::SONOBUOY_CLASS&&u.pos.range_to(area)<10.0).count()<8);
    }

    #[test]fn a_package_can_explicitly_choose_a_different_observed_target_without_false_identity(){
        let (mut staff,mut o,ids)=searching_package();
        let mut alternative=track(2,101.0);alternative.class_name=Some("Different observed ship".into());o.contacts.push(alternative);
        staff.refresh_strike_packages(&o);
        assert!(ids.iter().all(|id|staff.air_minds[id].target==Some(ContactId(2))));
        assert!(staff.take_notes().iter().any(|n|n.action=="strike_retarget"));
        assert_eq!(staff.track_memory[&ContactId(1)].1.class_name.as_deref(),Some("Known ship"));
    }
    #[test]fn current_reassociation_precedes_expiry_of_the_old_target_lease(){
        let (mut staff,mut o,ids)=searching_package();staff.decide(&o);staff.take_notes();
        o.tick=610;let mut alternative=track(2,101.0);alternative.class_name=Some("Different observed ship".into());o.contacts.push(alternative);
        let commands=staff.decide(&o);
        assert!(ids.iter().all(|id|staff.air_minds[id].target==Some(ContactId(2))));
        assert!(!commands.iter().any(|c|matches!(c,Command::ReturnToBase{unit}if ids.contains(unit))),"a new observed assignment must not be cancelled by the old expired lease");
    }
    #[test]fn shore_airbase_launches_without_surface_movement_and_aew_evades_observed_missile(){
        let db=Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
        let sc=harpoon_core::scenario_from_json(r#"{"id":"shore","name":"shore","duration_s":2000,"seed":7,
            "weather":{"visibility_nm":1,"jam":[0,0]},"context":{"air_wings":{"BASE":[{"class":"us_f18e","count":4},{"class":"us_e2d","count":1}]}},
            "units":[{"side":"Blue","class":"airfield","name":"BASE","pos":[0,0]}]}"#).unwrap();
        let mut w=harpoon_core::load_scenario(sc,db.clone()).unwrap();let base=w.entities.values().next().unwrap().id;
        let mut staff=TacticalStaff::with_db(Side::Blue,db.clone());let o=harpoon_core::observe(&w,Side::Blue,0,0);
        let commands=staff.decide(&o);
        assert!(commands.iter().any(|c|matches!(c,Command::LaunchAircraft{unit,..} if *unit==base)));
        assert!(!commands.iter().any(|c|matches!(c,Command::Follow{..}|Command::SetCourse{..}|Command::SetSpeed{..})),"ground hosts cannot receive fleet movement orders");
        harpoon_core::air::launch_aircraft(&mut w,base,"us_e2d",1,&AirMission::Recon {point:Vec2::new(10.0,0.0)}).unwrap();
        let mut o=harpoon_core::observe(&w,Side::Blue,0,0);o.tick=120;
        let air=o.units.iter().find(|u|u.kind==PlatformKind::Aircraft).unwrap().id;
        let mut m=track(99,10.0);m.category=PlatformKind::Missile;o.contacts.push(m);
        let commands=staff.decide(&o);
        assert!(commands.iter().any(|c|matches!(c,Command::SetAltitude{unit,altitude_m} if *unit==air && *altitude_m==150.0)));
        assert!(staff.take_notes().iter().any(|n|n.unit==air && n.action=="aew_evade"));
    }
    #[test]fn shore_bomber_sortie_uses_fuel_budget_instead_of_a_fixed_carrier_radius(){
        let db=Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
        let sc=harpoon_core::scenario_from_json(r#"{"id":"bomber-reach","name":"reach","duration_s":2000,"seed":7,
            "weather":{"visibility_nm":1,"jam":[0,0]},"context":{"air_wings":{"BASE":[{"class":"cn_h6j","count":4}]}},
            "units":[{"side":"Blue","class":"airfield","name":"BASE","pos":[0,0]}]}"#).unwrap();
        let w=harpoon_core::load_scenario(sc,db.clone()).unwrap();let mut o=harpoon_core::observe(&w,Side::Blue,0,0);
        let mut staff=TacticalStaff::with_db(Side::Blue,db);staff.params.cap_min_size=0;
        // Existing CAP is represented by two friendly observed aircraft.
        for i in 0..2 {let mut cap=o.units[0].clone();cap.id=EntityId(100+i);cap.kind=PlatformKind::Aircraft;cap.class="cn_j11b".into();cap.name=format!("CAP {i}");cap.mission=Some("cap".into());cap.weapons=serde_json::from_value(serde_json::json!([{"weapon":"cn_pl12","name":"PL-12","ammo":2,"cooldown_s":0}])).unwrap();cap.deck=None;o.units.push(cap);}
        o.contacts.push(track(1,400.0));
        let commands=staff.decide(&o);
        assert!(commands.iter().any(|c|matches!(c,Command::LaunchAircraft{class,mission:AirMission::Strike{..},..} if class=="cn_h6j")),"known in-budget shore target should be actionable");
        assert!(staff.strike_sortie_reach("cn_h6j")>400.0);
        o.contacts[0].estimate.x=10000.0;o.tick=1000;let mut staff=TacticalStaff::with_db(Side::Blue,staff.db.clone().unwrap());
        assert!(!staff.decide(&o).iter().any(|c|matches!(c,Command::LaunchAircraft{mission:AirMission::Strike{..},..})),"no unlimited-range sortie");
    }
}

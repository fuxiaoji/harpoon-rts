//! Arrival planning is commander policy. Actual range, channels, flight and
//! damage stay in core and may invalidate a plan. All inputs are own observations.
use super::*;

#[derive(Clone,Serialize,Deserialize)]
pub(super) struct PendingSalvo {
    unit:EntityId, contact:ContactId, weapon:String, rounds:u8, fire_at:i64, deadline:i64,
}

#[cfg(test)]mod tests {
 use super::*;
 #[test]fn a_ready_air_group_does_not_wait_for_a_hypothetical_surface_partner(){
  let db=Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
  let sc=harpoon_core::scenario_from_json(r#"{"id":"ready-air","name":"fixture","duration_s":1000,"seed":1,"context":{"air_wings":{"CV":[{"class":"us_f18e_asm","count":2}]}},"weather":{"visibility_nm":1,"jam":[0,0]},"units":[{"side":"Blue","class":"us_ford","name":"CV","pos":[0,0]}]}"#).unwrap();
  let mut w=harpoon_core::load_scenario(sc,db.clone()).unwrap();
  harpoon_core::air::launch_aircraft(&mut w,EntityId(1),"us_f18e_asm",2,&AirMission::Strike{contact:ContactId(1)}).unwrap();
  let mut o=harpoon_core::observe(&w,Side::Blue,0,0);
  o.contacts.push(serde_json::from_value(serde_json::json!({"id":1,"category":"Ship","estimate":{"x":60,"y":0},"est_course_deg":0,"est_speed_kts":0,"quality":0.8,"bearing_only":false,"last_seen_s":0,"stale":false,"sensors":[]})).unwrap());
  let ids:Vec<_>=o.units.iter_mut().filter(|u|u.kind==PlatformKind::Aircraft).map(|u|{u.pos=harpoon_core::Vec2::new(10.0,0.0);u.id}).collect();
  let shots=ids.iter().map(|id|Command::Fire{unit:*id,contact:ContactId(1),weapon:"us_agm84".into(),rounds:2}).collect();
  let mut staff=TacticalStaff::with_db(Side::Blue,db);
  let ready=staff.coordinate_salvos(&o,shots);
  assert_eq!(ready.iter().filter(|c|matches!(c,Command::Fire{..})).count(),2);
  assert!(staff.pending_salvos.is_empty());
  o.contacts[0].stale=true;o.tick=30;
  assert!(staff.coordinate_salvos(&o,vec![]).is_empty(),"no duplicate releases after the ready wave");
 }
 #[test]fn different_ranges_produce_one_common_arrival_and_pending_orders_survive_serialization(){
  let db=Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
  let sc=harpoon_core::scenario_from_json(r#"{"id":"arrival","name":"arrival","duration_s":2000,"seed":1,"weather":{"visibility_nm":1,"jam":[0,0]},"units":[{"side":"Blue","class":"cn_055","pos":[0,0]},{"side":"Blue","class":"cn_055","pos":[40,0]}]}"#).unwrap();
  let w=harpoon_core::load_scenario(sc,db.clone()).unwrap();let mut o=harpoon_core::observe(&w,Side::Blue,0,0);
  o.contacts.push(serde_json::from_value(serde_json::json!({"id":1,"category":"Ship","estimate":{"x":80,"y":0},"est_course_deg":0,"est_speed_kts":0,"quality":0.8,"bearing_only":false,"last_seen_s":0,"stale":false,"sensors":["radar_surf_med"]})).unwrap());
  let mut staff=TacticalStaff::with_db(Side::Blue,db.clone());
  let shots=vec![Command::Fire {unit:EntityId(1),contact:ContactId(1),weapon:"cn_yj18".into(),rounds:2},Command::Fire {unit:EntityId(2),contact:ContactId(1),weapon:"cn_yj18".into(),rounds:2}];
  assert!(staff.coordinate_salvos(&o,shots).is_empty());
  let encoded=serde_json::to_vec(&staff).unwrap();let mut restored:TacticalStaff=serde_json::from_slice(&encoded).unwrap();restored.restore_database(db.clone());
  let mut arrivals=Vec::new();
  for t in (30..=1200).step_by(30) {
   o.tick=t;let a=staff.coordinate_salvos(&o,vec![]);let b=restored.coordinate_salvos(&o,vec![]);assert_eq!(serde_json::to_value(&a).unwrap(),serde_json::to_value(&b).unwrap());
   for shot in a {if let Command::Fire {unit,weapon,..}=shot {let u=o.units.iter().find(|u|u.id==unit).unwrap();arrivals.push(t as f64+u.pos.range_to(o.contacts[0].estimate)/db.weapons[&weapon].speed_kts*3600.0);}}
  }
  assert_eq!(arrivals.len(),2,"each scheduled shooter fires once");assert!((arrivals[0]-arrivals[1]).abs()<=30.0,"different launch ranges should meet within one 30-second staff interval");
 }
 #[test]fn stale_track_holds_until_reacquisition_but_rtb_cancels(){
  let db=Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
  let sc=harpoon_core::scenario_from_json(r#"{"id":"hold","name":"hold","duration_s":1000,"seed":1,"weather":{"visibility_nm":1,"jam":[0,0]},"units":[{"side":"Blue","class":"cn_055","pos":[0,0]}]}"#).unwrap();
  let w=harpoon_core::load_scenario(sc,db.clone()).unwrap();let mut o=harpoon_core::observe(&w,Side::Blue,0,0);
  o.contacts.push(serde_json::from_value(serde_json::json!({"id":1,"category":"Ship","estimate":{"x":30,"y":0},"est_course_deg":0,"est_speed_kts":0,"quality":0.8,"bearing_only":false,"last_seen_s":0,"stale":true,"sensors":[]})).unwrap());
  let mut staff=TacticalStaff::with_db(Side::Blue,db);
  let p=PendingSalvo {unit:o.units[0].id,contact:ContactId(1),weapon:"cn_yj18".into(),rounds:2,fire_at:60,deadline:300};
  staff.pending_salvos.push(p.clone());o.tick=90;
  assert!(staff.coordinate_salvos(&o,vec![]).is_empty());assert_eq!(staff.pending_salvos.len(),1);
  o.tick=120;o.contacts[0].stale=false;
  assert_eq!(staff.coordinate_salvos(&o,vec![]).len(),1);assert!(staff.pending_salvos.is_empty());
  assert!(staff.coordinate_salvos(&o,vec![]).is_empty(),"release is consumed once");
  staff.pending_salvos.push(p.clone());o.units[0].mission=Some("rtb".into());
  assert!(staff.coordinate_salvos(&o,vec![]).is_empty());assert!(staff.pending_salvos.is_empty());
  staff.pending_salvos.push(p);o.units[0].mission=None;o.tick=301;o.contacts[0].stale=true;
  assert!(staff.coordinate_salvos(&o,vec![]).is_empty());assert!(staff.pending_salvos.is_empty(),"stale plan still expires");
 }
 #[test]fn a_valid_committed_target_is_sticky_and_late_members_join_its_window(){
  let db=Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
  let sc=harpoon_core::scenario_from_json(r#"{"id":"stable-window","name":"fixture","duration_s":1500,"seed":1,"weather":{"visibility_nm":1,"jam":[0,0]},"units":[{"side":"Blue","class":"cn_055","pos":[0,0]},{"side":"Blue","class":"cn_055","pos":[20,0]}]}"#).unwrap();
  let w=harpoon_core::load_scenario(sc,db.clone()).unwrap();let mut o=harpoon_core::observe(&w,Side::Blue,0,0);
  o.contacts.push(serde_json::from_value(serde_json::json!({"id":1,"category":"Ship","estimate":{"x":80,"y":0},"est_course_deg":0,"est_speed_kts":0,"quality":0.8,"bearing_only":false,"last_seen_s":0,"stale":false,"sensors":[]})).unwrap());let mut other=o.contacts[0].clone();other.id=ContactId(2);o.contacts.push(other);
  let mut staff=TacticalStaff::with_db(Side::Blue,db);
  staff.coordinate_salvos(&o,vec![Command::Fire{unit:EntityId(1),contact:ContactId(1),weapon:"cn_yj18".into(),rounds:2}]);let original=staff.pending_salvos[0].fire_at;
  o.tick=30;staff.coordinate_salvos(&o,vec![Command::Fire{unit:EntityId(1),contact:ContactId(2),weapon:"cn_yj18".into(),rounds:2},Command::Fire{unit:EntityId(2),contact:ContactId(1),weapon:"cn_yj18".into(),rounds:2}]);
  assert!(staff.pending_salvos.iter().all(|p|p.contact==ContactId(1)));assert_eq!(staff.pending_salvos.iter().find(|p|p.unit==EntityId(1)).unwrap().fire_at,original);
  let arrivals:Vec<_>=staff.pending_salvos.iter().map(|p|{let u=o.units.iter().find(|u|u.id==p.unit).unwrap();p.fire_at as f64+u.pos.range_to(o.contacts[0].estimate)/staff.db.as_ref().unwrap().weapons[&p.weapon].speed_kts*3600.0}).collect();assert!((arrivals[0]-arrivals[1]).abs()<=30.0);
  o.contacts[0].stale=true;o.tick=40;staff.coordinate_salvos(&o,vec![Command::Fire{unit:EntityId(1),contact:ContactId(2),weapon:"cn_yj18".into(),rounds:2}]);assert!(staff.pending_salvos.iter().any(|p|p.unit==EntityId(1)&&p.contact==ContactId(2)));
 }
 #[test]fn shore_and_ship_cruise_orders_use_the_same_observed_target_scheduler(){
  let mut db=Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
  let mut battery=db.platforms["navsta"].clone();battery.weapons=db.platforms["cn_055"].weapons.iter().filter(|m|m.weapon=="cn_dh10").cloned().collect();db.platforms.insert("test_cruise_battery".into(),battery);
  let sc=harpoon_core::scenario_from_json(r#"{"id":"joint-schedule-fixture","name":"fixture","duration_s":1500,"seed":2,"weather":{"visibility_nm":1,"jam":[0,0]},"units":[{"side":"Blue","class":"test_cruise_battery","pos":[0,0]},{"side":"Blue","class":"cn_055","pos":[30,0]}]}"#).unwrap();
  let w=harpoon_core::load_scenario(sc,db.clone()).unwrap();let mut o=harpoon_core::observe(&w,Side::Blue,0,0);
  o.contacts.push(serde_json::from_value(serde_json::json!({"id":1,"category":"Base","estimate":{"x":80,"y":0},"quality":0.8,"bearing_only":false,"last_seen_s":0,"stale":false,"sensors":[],"est_course_deg":0,"est_speed_kts":0})).unwrap());
  let mut staff=TacticalStaff::with_db(Side::Blue,db.clone());let shots=o.units.iter().map(|u|Command::Fire{unit:u.id,contact:ContactId(1),weapon:"cn_dh10".into(),rounds:2}).collect();
  assert!(staff.coordinate_salvos(&o,shots).is_empty());assert_eq!(staff.pending_salvos.len(),2);
  let mut arrivals=Vec::new();for tick in (30..=1200).step_by(30){o.tick=tick;for c in staff.coordinate_salvos(&o,vec![]){if let Command::Fire{unit,weapon,..}=c{let u=o.units.iter().find(|u|u.id==unit).unwrap();arrivals.push(tick as f64+u.pos.range_to(o.contacts[0].estimate)/db.weapons[&weapon].speed_kts*3600.0);}}}
  assert_eq!(arrivals.len(),2);assert!((arrivals[0]-arrivals[1]).abs()<=30.0);
 }
}

impl TacticalStaff {
    pub(super) fn coordinate_salvos(&mut self,obs:&Observation,commands:Vec<Command>)->Vec<Command> {
        let mut out=Vec::new();let mut groups:BTreeMap<ContactId,Vec<(EntityId,String,u8,f64)>>=BTreeMap::new();
        for cmd in commands {
            if let Command::Fire {unit,contact,weapon,rounds}=&cmd {
                if matches!(self.kind_of(weapon),Some(WeaponKind::Ascm|WeaponKind::Slcm)) {
                    let committed=self.pending_salvos.iter().find(|p|p.unit==*unit&&p.weapon==*weapon&&p.contact!=*contact&&p.deadline>=obs.tick)
                        .is_some_and(|p|obs.contacts.iter().any(|c|c.id==p.contact&&!c.stale&&!c.bearing_only&&c.quality>=self.params.strike_min_q&&c.bda_hp_fraction!=Some(0.0))&&self.supervision.target_allowed(p.contact,obs.tick));
                    if committed {continue;} // Do not churn a still-valid committed window.
                    let replaced=self.pending_salvos.iter().any(|p|p.unit==*unit && p.weapon==*weapon && p.contact!=*contact);
                    self.pending_salvos.retain(|p|p.unit!=*unit || p.weapon!=*weapon || p.contact==*contact);
                    if replaced {self.note(*unit,Some(*contact),"salvo_retarget","重新指定目标，取消该平台尚未发射的旧目标计划".into(),"new target replaces this platform's unreleased salvo plan".into());}
                    if self.pending_salvos.iter().any(|p|p.unit==*unit && p.weapon==*weapon) {continue;}
                    if let (Some(u),Some(c),Some(w))=(obs.units.iter().find(|u|u.id==*unit),obs.contacts.iter().find(|c|c.id==*contact),self.db.as_ref().and_then(|db|db.weapons.get(weapon))) {
                        let eta=harpoon_core::rules::damage_runtime::flight_seconds(w,u.pos.range_to(c.estimate));
                        groups.entry(*contact).or_default().push((*unit,weapon.clone(),*rounds,eta));continue;
                    }
                }
            }
            out.push(cmd);
        }
        for (contact,shots) in groups {
            if shots.len()==1&&!self.pending_salvos.iter().any(|p|p.contact==contact)&&obs.units.iter().any(|u|u.id==shots[0].0&&u.kind==PlatformKind::Aircraft){
                let (unit,weapon,rounds,_)=shots.into_iter().next().unwrap();
                out.push(Command::Fire{unit,contact,weapon,rounds});
                self.note(unit,Some(contact),"single_release","单个有效释放单元，无其他可加入的发射计划，立即提交核心复核".into(),"Single ready release element with no committed partners; submit to core without a fictitious coordination wait".into());continue;
            }
            let latest=shots.iter().map(|(_,_,_,eta)|*eta).fold(0.0,f64::max);
            let all_air=shots.iter().all(|(id,_,_,_)|obs.units.iter().any(|u|u.id==*id&&u.kind==PlatformKind::Aircraft));
            // An already ready air group has no reason to loiter for a
            // hypothetical ship. Only shorter flights wait for real partners.
            let slots=if all_air {(latest/30.0).round()}else{(latest/30.0).ceil()};
            let proposed=obs.tick+(slots as i64)*30+if all_air {0}else{60};
            let existing=self.pending_salvos.iter().filter(|p|p.contact==contact&&p.fire_at>obs.tick).filter_map(|p| {
                let u=obs.units.iter().find(|u|u.id==p.unit)?;let c=obs.contacts.iter().find(|c|c.id==contact)?;
                Some(p.fire_at+harpoon_core::rules::damage_runtime::flight_seconds(self.db.as_ref()?.weapons.get(&p.weapon)?,u.pos.range_to(c.estimate)).round() as i64)
            }).max();
            let arrival=existing.filter(|at|*at>=obs.tick+latest.ceil() as i64+15&&*at<=proposed+120).unwrap_or(proposed);
            for (unit,weapon,rounds,eta) in shots {
                self.pending_salvos.push(PendingSalvo {unit,contact,weapon,rounds,fire_at:arrival-((eta/30.0).round() as i64)*30,deadline:arrival+120});
                self.note(unit,Some(contact),"salvo_schedule",format!("向C{}编排共同抵达窗口T{}；等待本机发射时刻",contact.0,arrival),format!("schedule C{} common arrival T{}; hold until own release time",contact.0,arrival));
            }
        }
        let pending=std::mem::take(&mut self.pending_salvos);
        for p in pending {
            let Some(u)=obs.units.iter().find(|u|u.id==p.unit) else {continue};
            if obs.tick>p.deadline || u.mission.as_deref()==Some("rtb") || !u.weapons.iter().any(|w|w.weapon==p.weapon && w.ammo>0) {
                self.note(p.unit,Some(p.contact),"salvo_cancel",format!("计划发射取消：超时、返航或弹药已不可用（C{}）",p.contact.0),format!("scheduled salvo cancelled: expired, RTB or ammunition unavailable (C{})",p.contact.0));
                continue;
            }
            let Some(c)=obs.contacts.iter().find(|c|c.id==p.contact && !c.stale && !c.bearing_only && c.bda_hp_fraction!=Some(0.0)) else {
                self.note(p.unit,Some(p.contact),"salvo_hold_track",format!("计划发射暂缓：C{}无有效航迹，保留至计划期限",p.contact.0),format!("scheduled salvo held: C{} lacks a valid track; wait within deadline",p.contact.0));
                self.pending_salvos.push(p);continue;
            };
            if obs.tick>=p.fire_at && u.pos.range_to(c.estimate)<=self.range_of(&p.weapon)*0.98 {
                out.push(Command::Fire {unit:p.unit,contact:p.contact,weapon:p.weapon,rounds:p.rounds});
                self.note(p.unit,Some(p.contact),"salvo_release",format!("向C{}执行计划发射；以实际武器回执确认",p.contact.0),format!("execute scheduled release at C{}; confirm actual weapon receipt",p.contact.0));
            } else {self.pending_salvos.push(p);}
        }
        out
    }
}

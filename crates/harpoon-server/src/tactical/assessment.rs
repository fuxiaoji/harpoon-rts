//! Game decision support from a side's observation and public class data.
//! Nominal coverage is a planning estimate, never a guaranteed interception.
use super::*;
use harpoon_core::Vec2;
use serde_json::{json, Value};
pub fn danger(obs: &Observation, db: &Database, point: Vec2) -> bool {
    obs.contacts
        .iter()
        .filter(|c| !c.stale && !c.bearing_only && c.quality >= 0.3)
        .any(|c| {
            let radius = match c.category {
                PlatformKind::Missile => 45.0,
                PlatformKind::Aircraft => 40.0,
                PlatformKind::Ship => c
                    .class_name
                    .as_ref()
                    .and_then(|name| db.platforms.values().find(|p| &p.name == name))
                    .map(|p| {
                        p.weapons
                            .iter()
                            .filter_map(|m| db.weapons.get(&m.weapon))
                            .filter(|w| w.kind == WeaponKind::Sam)
                            .map(|w| w.range_nm)
                            .fold(0.0, f64::max)
                    })
                    .unwrap_or(45.0),
                _ => 0.0,
            };
            radius > 0.0 && point.range_to(c.estimate) < radius
        })
}
pub fn covered(obs: &Observation, db: &Database, point: Vec2) -> bool {
    obs.units.iter().any(|u| {
            matches!(u.kind, PlatformKind::Ship | PlatformKind::Base)&&u.systems.fire_control>0.05
            && u.weapons.iter().any(|m| {
                db.weapons.get(&m.weapon).is_some_and(|w| {
                    w.kind == WeaponKind::Sam
                        && m.ammo >= 4
                        && point.range_to(u.pos) < w.range_nm * 0.75
                })
            })
    })
}
pub fn station(obs: &Observation, db: &Database, home: &UnitObs, desired: Vec2) -> Vec2 {
    let axis = home.pos.bearing_to(desired);
    let mut points = vec![desired, home.pos];
    points.extend(
        [0.0, 60.0, -60.0, 120.0, -120.0, 180.0]
            .map(|offset| home.pos.advanced(axis + offset, 25.0)),
    );
    points.into_iter().min_by(|a,b| {
        let score=|p:Vec2|if danger(obs,db,p){10000.0}else{0.0}+if covered(obs,db,p){0.0}else{100.0}+p.range_to(desired);
        score(*a).total_cmp(&score(*b))
    }).unwrap_or(home.pos)
}
/// Search sectors use the same observed buoy coverage before launch and while
/// airborne. A covered sector is not a reason to keep recycling helicopters.
pub fn asw_station(obs:&Observation,db:&Database,home:&UnitObs,offset:u32)->Option<Vec2> {
    (0..4).map(|i|home.pos.advanced(home.course_deg+((offset+i)%4) as f64*90.0,12.0))
        .find(|p|!danger(obs,db,*p)&&obs.units.iter().filter(|u|
            u.class==harpoon_core::rules::asw::SONOBUOY_CLASS&&u.pos.range_to(*p)<10.0).count()<8)
}
pub fn brief(obs: &Observation, db: &Database, supervision: &Value) -> Value {
    let hosts:Vec<_>=obs.units.iter().filter(|u|u.deck.is_some()).map(|h| {
        let sam:u32=obs.units.iter().filter(|s|s.pos.range_to(h.pos)<35.0).flat_map(|s|&s.weapons).filter(|m|db.weapons.get(&m.weapon).is_some_and(|w|w.kind==WeaponKind::Sam)).map(|m|m.ammo as u32).sum();
        let cap=obs.units.iter().filter(|u|u.mission.as_deref()==Some("cap")&&u.pos.range_to(h.pos)<45.0).count();
        let support:Vec<_>=obs.units.iter().filter(|u|u.kind==PlatformKind::Aircraft&&u.mission.as_deref()==Some("recon")&&u.pos.range_to(h.pos)<90.0).map(|u|json!({"unit":u.id,"needs_handover":u.endurance_s.unwrap_or(0.0)<u.pos.range_to(h.pos)/db.platforms.get(&u.class).map(|p|p.cruise_speed_kts.max(60.0)).unwrap_or(300.0)*3600.0*1.3+900.0,"known_station_risk":danger(obs,db,u.pos)})).collect();
        json!({"host":h.id,"nearby_SAM_stock":sam,"local_CAP":cap,"nominal_cover":covered(obs,db,h.pos),"known_support_risk":danger(obs,db,h.pos),"support_handover":support})
    }).collect();
    let targets:Vec<_>=obs.contacts.iter().filter(|c|c.category==PlatformKind::Ship&&!known_sensor_contact(c)&&!c.stale&&!c.bearing_only&&c.quality>=0.4&&c.bda_hp_fraction!=Some(0.0)).map(|c| {
        let hold=supervision["attack_account"][c.id.0.to_string()]["hold_until"].as_i64().unwrap_or(0)>obs.tick;
        json!({"contact":c.id,"quality":c.quality,"own_in_flight":c.own_in_flight,"confirmed_hits":c.bda_hits,"confirmed_damage":c.bda_damage,"repeat_attack_held":hold,
            "option":if hold{"recon_or_another_fresh_target"}else if c.own_in_flight>0{"await_current_attack_feedback"}else{"check_cover_fuel_support_and_shared_release_window"}})
    }).collect();
    json!({"basis":"own observations and nominal public data; not hidden enemy readiness","hosts":hosts,"observed_target_options":targets,
        "plan_checks":["CAP station + supporting SAM stock + ready replacement must be budgeted together","keep one safe radar relay while the strike elements descend","bind EW to a named package, reassess station and replacement before launch","joint releases must have a current firing track, fuel reserve and cancel deadline","after an unconfirmed attack, change an observed assignment or gather feedback before repeating"],
        "effect_evaluation":"unknown BDA is unknown, not proof of a miss; decisions require actual expenditure and own receipts"})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nominal_cover_and_known_risk_are_separate_and_advice_has_no_hidden_truth() {
        let db =
            Database::from_json(include_str!("../../../../database/consolidated.json")).unwrap();
        let sc=harpoon_core::scenario_from_json(r#"{"id":"assessment","name":"test","duration_s":1000,"seed":1,"weather":{"visibility_nm":1,"jam":[0,0]},"units":[{"side":"Blue","class":"cn_055","pos":[0,0]},{"side":"Red","class":"us_ford","pos":[150,0]}]}"#).unwrap();
        let mut w = harpoon_core::load_scenario(sc, db.clone()).unwrap();
        let o = harpoon_core::observe(&w, Side::Blue, 0, 0);
        assert!(covered(&o, &db, harpoon_core::Vec2::zero()));
        assert!(!danger(&o, &db, harpoon_core::Vec2::zero()));
        let a = brief(&o, &db, &Value::Null);
        w.entity_mut(EntityId(2)).unwrap().pos = harpoon_core::Vec2::zero();
        let o2 = harpoon_core::observe(&w, Side::Blue, 0, 0);
        assert_eq!(a, brief(&o2, &db, &Value::Null));
        let mut known = o.clone();
        known.contacts.push(serde_json::from_value(json!({"id":1,"category":"Aircraft","estimate":{"x":50,"y":0},"quality":0.8,"stale":false,"bearing_only":false,"last_seen_s":0,"est_course_deg":0,"est_speed_kts":0,"sensors":[]})).unwrap());
        let point = station(
            &known,
            &db,
            &known.units[0],
            harpoon_core::Vec2::new(45.0, 0.0),
        );
        assert!(!danger(&known, &db, point));
        assert!(!a.to_string().contains("about"));
        let mut buoy=known.contacts[0].clone();buoy.id=ContactId(9);buoy.category=PlatformKind::Ship;buoy.class_name=Some("AN/SSQ Sonobuoy".into());known.contacts.push(buoy);
        assert!(brief(&known,&db,&Value::Null)["observed_target_options"].as_array().unwrap().is_empty(),"identified buoys are sensing objects, not strike options");
    }
}

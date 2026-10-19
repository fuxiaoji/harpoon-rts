//! A bounded, deterministic exercise opponent. Receives only its Observation.
use harpoon_core::{AirMission, Command, Observation, Side};
use harpoon_core::types::PlatformKind;

pub fn orders(obs: &Observation) -> Vec<(Side, Command)> {
    let next_tick = obs.tick + 1;
    if obs.over || next_tick % 30 != 0 { return Vec::new(); }
    let mut out = Vec::new();
    for unit in obs.units.iter().filter(|u| u.kind == PlatformKind::Ship) {
        let Some(deck) = &unit.deck else { continue };
        let ready = |class: &str| deck.ready.iter().find(|(c, _)| c == class).map(|(_, n)| *n).unwrap_or(0);
        let command = if next_tick == 30 && ready("us_f18e") >= 2 {
            Some(Command::LaunchAircraft { unit: unit.id, class: "us_f18e".into(), count: 2,
                mission: AirMission::Cap { station: unit.pos, radius_nm: 25.0 } })
        } else if next_tick == 60 && ready("us_mh60r") >= 2 {
            Some(Command::LaunchAircraft { unit: unit.id, class: "us_mh60r".into(), count: 2,
                mission: AirMission::Asw { area: unit.pos, radius_nm: 15.0 } })
        } else if next_tick == 90 && ready("us_f18e_asm") >= 2 && obs.mission.is_some() {
            // Scout a publicly briefed exercise area; never an unseen enemy coordinate.
            Some(Command::LaunchAircraft { unit: unit.id, class: "us_f18e_asm".into(), count: 2,
                mission: AirMission::Recon { point: obs.mission.as_ref().unwrap().config.center } })
        } else if next_tick >= 120 && next_tick <= 3600 && (next_tick - 120) % 900 == 0 && ready("us_f18e_asm") >= 2 {
            let target = obs.contacts.iter().filter(|c| !c.stale && !c.bearing_only && c.quality > 0.4 && c.category == PlatformKind::Ship)
                .min_by(|a,b| unit.pos.range_to(a.estimate).total_cmp(&unit.pos.range_to(b.estimate)));
            target.map(|c| Command::LaunchAircraft { unit: unit.id, class: "us_f18e_asm".into(), count: 2,
                mission: AirMission::Strike { contact: c.id } })
        } else { None };
        if let Some(c) = command { out.push((obs.your_side, c)); }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_contact_produces_no_strike_and_only_ready_aircraft_can_launch() {
        let db: harpoon_core::Database = harpoon_core::Database::from_json(include_str!("../../../database/consolidated.json")).unwrap();
        let sc = harpoon_core::scenario_from_json(include_str!("../../../scenarios/dual_carrier_denial.json")).unwrap();
        let mut world = harpoon_core::load_scenario(sc, db).unwrap();
        world.tick = 119;
        assert!(orders(&harpoon_core::observe(&world, Side::Red, 0, 0)).is_empty());
        world.tick = 29;
        let cmds = orders(&harpoon_core::observe(&world, Side::Red, 0, 0));
        assert_eq!(cmds.len(), 2);
        assert!(cmds.iter().all(|(side, c)| *side == Side::Red && matches!(c, Command::LaunchAircraft {class, ..} if class == "us_f18e")));
        world.tick = 30;
        assert!(orders(&harpoon_core::observe(&world, Side::Red, 0, 0)).is_empty());
        world.tick = 89;
        let scouting=orders(&harpoon_core::observe(&world, Side::Red, 0, 0));
        assert_eq!(scouting.len(),2);
        let center=world.context.area_denial.as_ref().unwrap().center;
        assert!(scouting.iter().all(|(_,c)| matches!(c,Command::LaunchAircraft {class,mission:AirMission::Recon{point},..} if class=="us_f18e_asm" && *point==center)));
    }
}

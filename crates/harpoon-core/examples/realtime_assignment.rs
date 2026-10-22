//! Small finite-action task allocation diagnostic, scored by the production core.
//! Exhaustive enumeration is an offline reference; its runtime is not free.
use harpoon_core::{
    load_scenario, observe, sim, Command, Database, EntityId, Rng, Scenario, Side, Vec2,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    time::Instant,
};

const PUBLICATION: i64 = 160;
const HORIZON: i64 = 1360;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Task {
    id: usize,
    goal: Vec2,
    deadline_tick: i64,
    reward: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Route {
    unit: u32,
    task_ids: Vec<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Plan {
    routes: Vec<Route>,
    #[serde(default)]
    brief: String,
}
#[derive(Clone)]
struct Case {
    scenario: Scenario,
    tasks: Vec<Task>,
    start_tick: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Evaluation {
    plan: Plan,
    delay_s: i64,
    earned: u32,
    total: u32,
    reached: BTreeMap<usize, i64>,
    world_hash: String,
}

fn case(seed: u64, tight: bool) -> Case {
    let scenario: Scenario = serde_json::from_value(json!({
        "id":"research_assignment_v1", "name":"Public navigation allocation", "duration_s":HORIZON,
        "description":"Visit assigned public task zones; one vessel per task, at most two ordered tasks per vessel.",
        "seed":seed, "weather":{"visibility_nm":12,"jam":[0,0]}, "victory":{"target_vp":0},
        "units":[
            {"side":"Blue","class":"ddg_aegis","name":"V1","pos":{"x":-1,"y":-0.6},"course_deg":90,"speed_kts":18,"waypoints":[{"x":8,"y":-0.6}],"fire_on":false},
            {"side":"Blue","class":"ddg_aegis","name":"V2","pos":{"x":-1,"y":0.4},"course_deg":90,"speed_kts":18,"waypoints":[{"x":8,"y":0.4}],"fire_on":false},
            {"side":"Blue","class":"ddg_aegis","name":"V3","pos":{"x":-1,"y":1.4},"course_deg":90,"speed_kts":18,"waypoints":[{"x":8,"y":1.4}],"fire_on":false},
            {"side":"Red","class":"ddg_aegis","name":"DISTANT CONTROL","pos":{"x":5000,"y":5000},"fire_on":false}
        ]})).unwrap();
    let mut rng = Rng::new(seed).derive(0x61737369676e);
    let mut rewards = vec![1, 2, 3, 4];
    for i in (1..4).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        rewards.swap(i, j);
    }
    let windows = [180, 240, 360, 480];
    let tasks = (0..4)
        .map(|id| Task {
            id,
            goal: Vec2::new(rng.range(-0.1, 1.8), rng.range(-0.8, 1.6)),
            deadline_tick: PUBLICATION + windows[id] + if tight { 0 } else { 300 },
            reward: rewards[id],
        })
        .collect();
    Case {
        scenario,
        tasks,
        start_tick: 0,
    }
}

/// Restricted public-state reconstruction for these intact navigation tasks.
/// This contains no opponent, judge seed, future update, or judge World clone.
fn from_public(input: &serde_json::Value) -> Result<Case, String> {
    let obs = &input["observation"];
    if obs["tick"].as_i64() != Some(PUBLICATION) {
        return Err("this diagnostic expects the publication snapshot".into());
    }
    let own = obs["units"].as_array().ok_or("missing own units")?;
    if own.len() != 3 {
        return Err("expected three intact own ships".into());
    }
    let mut units = vec![];
    for (index, unit) in own.iter().enumerate() {
        if unit["id"].as_u64() != Some(index as u64 + 1)
            || unit["side"] != "Blue"
            || unit["class"] != "ddg_aegis"
            || unit["hp"] != unit["hp_max"]
            || unit["systems"]
                .as_object()
                .is_none_or(|s| s.values().any(|v| v.as_f64() != Some(1.0)))
        {
            return Err(
                "public reconstruction is restricted to this intact navigation class".into(),
            );
        }
        let order = input["standing_orders"]
            .as_array()
            .ok_or("missing standing orders")?
            .iter()
            .find(|r| r["unit"] == unit["id"])
            .ok_or("missing own route")?;
        units.push(
            json!({"side":"Blue", "class":unit["class"], "name":unit["name"],
            "pos":unit["pos"], "course_deg":unit["course_deg"], "speed_kts":unit["speed_kts"],
            "waypoints":order["waypoints"], "fire_on":false}),
        );
    }
    let scenario = serde_json::from_value(json!({"id":"public_navigation_reconstruction",
        "name":"Public navigation reconstruction", "duration_s":HORIZON,
        "seed":0, "weather":{"visibility_nm":12,"jam":[0,0]},
        "victory":{"target_vp":0}, "units":units}))
    .map_err(|e| e.to_string())?;
    let tasks: Vec<Task> =
        serde_json::from_value(input["public_tasks"].clone()).map_err(|e| e.to_string())?;
    if tasks.len() != 4 || tasks.iter().enumerate().any(|(id, t)| t.id != id) {
        return Err("expected four ordered public task ids".into());
    }
    Ok(Case {
        scenario,
        tasks,
        start_tick: PUBLICATION,
    })
}

fn empty_plan() -> Plan {
    Plan {
        routes: (1..=3)
            .map(|unit| Route {
                unit,
                task_ids: vec![],
            })
            .collect(),
        brief: String::new(),
    }
}

fn plans() -> Vec<Plan> {
    fn visit(plan: &mut Plan, seen: &mut BTreeMap<String, Plan>) {
        let key = serde_json::to_string(&plan.routes).unwrap();
        if seen.insert(key, plan.clone()).is_some() {
            return;
        }
        let used: BTreeSet<_> = plan
            .routes
            .iter()
            .flat_map(|r| r.task_ids.iter().copied())
            .collect();
        for task in 0..4 {
            if used.contains(&task) {
                continue;
            }
            for route in 0..3 {
                if plan.routes[route].task_ids.len() == 2 {
                    continue;
                }
                plan.routes[route].task_ids.push(task);
                visit(plan, seen);
                plan.routes[route].task_ids.pop();
            }
        }
    }
    let mut seen = BTreeMap::new();
    visit(&mut empty_plan(), &mut seen);
    seen.into_values().collect()
}

fn validate(p: &Plan) -> Result<(), String> {
    if p.routes.len() != 3 {
        return Err("provide exactly three vessel routes".into());
    }
    let mut units = BTreeSet::new();
    let mut tasks = BTreeSet::new();
    for r in &p.routes {
        if !(1..=3).contains(&r.unit) || !units.insert(r.unit) || r.task_ids.len() > 2 {
            return Err("invalid or duplicate vessel route".into());
        }
        for task in &r.task_ids {
            if *task >= 4 || !tasks.insert(*task) {
                return Err("invalid or duplicate task".into());
            }
        }
    }
    Ok(())
}

fn public_input(c: &Case) -> serde_json::Value {
    let mut w = load_scenario(c.scenario.clone(), Database::built_in()).unwrap();
    w.tick = c.start_tick;
    for _ in c.start_tick..PUBLICATION {
        sim::tick(&mut w, &[]);
    }
    let old: Vec<_> = (1..=3)
        .map(|id| json!({"unit":id,"waypoints":w.entity(EntityId(id)).unwrap().waypoints}))
        .collect();
    json!({"observation":observe(&w,Side::Blue,0,32),"public_tasks":c.tasks,"standing_orders":old,
        "rules":{"max_tasks_per_vessel":2,"each_task_at_most_once":true,"goal_radius_nm":0.12,
            "speed_kts":18,"turn_rate_deg_s":1.8,"approach_speed_cap_kts":15,"approach_distance_nm":0.5,
            "waypoint_pop_radius_nm":0.1,"task_scoring":"Assigned tasks are checked in route order; a late arrival earns zero but navigation continues.",
            "unassigned":"Empty route continues the old standing order; unassigned tasks do not score."}})
}

fn evaluate(c: &Case, p: &Plan, delay_s: i64) -> Result<Evaluation, String> {
    validate(p)?;
    if !(0..=900).contains(&delay_s) {
        return Err("delay must be 0..900".into());
    }
    let mut w = load_scenario(c.scenario.clone(), Database::built_in())?;
    w.tick = c.start_tick;
    let mut progress = [0usize; 3];
    let mut reached = BTreeMap::new();
    let mut earned = 0;
    let apply = PUBLICATION + delay_s + 1;
    for tick in (c.start_tick + 1)..=HORIZON {
        let mut batch = vec![];
        if tick == apply {
            for r in &p.routes {
                if !r.task_ids.is_empty() {
                    batch.push((
                        Side::Blue,
                        Command::Goto {
                            unit: EntityId(r.unit),
                            waypoints: r.task_ids.iter().map(|id| c.tasks[*id].goal).collect(),
                        },
                    ));
                    batch.push((
                        Side::Blue,
                        Command::SetSpeed {
                            unit: EntityId(r.unit),
                            speed_kts: 18.0,
                        },
                    ));
                }
            }
        }
        sim::tick(&mut w, &batch);
        if tick < apply {
            continue;
        }
        for (index, r) in p.routes.iter().enumerate() {
            if let Some(id) = r.task_ids.get(progress[index]) {
                let task = &c.tasks[*id];
                if w.entity(EntityId(r.unit)).unwrap().pos.range_to(task.goal) <= 0.12 {
                    reached.insert(*id, tick);
                    if tick <= task.deadline_tick {
                        earned += task.reward;
                    }
                    progress[index] += 1;
                }
            }
        }
    }
    let mut state_hash = 0xcbf29ce484222325u64;
    for b in serde_json::to_vec(&w).unwrap() {
        state_hash = (state_hash ^ b as u64).wrapping_mul(0x100000001b3);
    }
    Ok(Evaluation {
        plan: p.clone(),
        delay_s,
        earned,
        total: c.tasks.iter().map(|t| t.reward).sum(),
        reached,
        world_hash: format!("{state_hash:016x}"),
    })
}

fn greedy(c: &Case) -> Plan {
    let input = public_input(c);
    let obs = &input["observation"]["units"];
    let mut positions: Vec<Vec2> = serde_json::from_value::<Vec<serde_json::Value>>(obs.clone())
        .unwrap()
        .iter()
        .map(|u| serde_json::from_value(u["pos"].clone()).unwrap())
        .collect();
    let mut times = [PUBLICATION as f64; 3];
    let mut p = empty_plan();
    let mut used = BTreeSet::new();
    loop {
        let mut best: Option<(f64, usize, usize, f64)> = None;
        for unit in 0..3 {
            if p.routes[unit].task_ids.len() == 2 {
                continue;
            }
            for task in &c.tasks {
                if used.contains(&task.id) {
                    continue;
                }
                // Cheap estimate only; actual scores always come from the core.
                let travel = positions[unit].range_to(task.goal) / 18.0 * 3600.0 + 30.0;
                if times[unit] + travel > task.deadline_tick as f64 {
                    continue;
                }
                let priority = task.reward as f64 / travel.max(1.0);
                if best.as_ref().is_none_or(|b| priority > b.0) {
                    best = Some((priority, unit, task.id, travel));
                }
            }
        }
        let Some((_, unit, id, travel)) = best else {
            break;
        };
        p.routes[unit].task_ids.push(id);
        used.insert(id);
        positions[unit] = c.tasks[id].goal;
        times[unit] += travel;
    }
    p.brief = "Cheap reward/travel greedy; estimate ignores exact turn dynamics.".into();
    p
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |key: &str| args.windows(2).find(|a| a[0] == key).map(|a| a[1].clone());
    let seed = value("--seed")
        .unwrap_or_else(|| "0".into())
        .parse::<u64>()
        .map_err(|e| e.to_string())?;
    let family = value("--case").unwrap_or_else(|| "d3".into());
    if family != "d3" && family != "d4" {
        return Err("case must be d3 or d4".into());
    }
    let c = if let Some(path) = value("--public-input") {
        let input = serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        from_public(&input)?
    } else {
        case(seed, family == "d4")
    };
    let delay = value("--delay-s")
        .unwrap_or_else(|| "0".into())
        .parse::<i64>()
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    let result = if args.iter().any(|a| a == "--emit-input") {
        public_input(&c)
    } else if args.iter().any(|a| a == "--oracle") {
        let all = plans();
        let scores: Vec<_> = all
            .iter()
            .map(|p| evaluate(&c, p, delay).unwrap())
            .collect();
        let best = scores.iter().max_by_key(|s| s.earned).unwrap();
        json!({"family":family,"seed":seed,"delay_s":delay,"candidate_count":all.len(),"best":best,
            "scores":scores,"reference":"offline exact optimum within this finite route class; computation time is not free"})
    } else {
        let planning_start = Instant::now();
        let external_plan = value("--plan");
        let p = if let Some(path) = &external_plan {
            serde_json::from_str::<Plan>(&fs::read_to_string(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
        } else {
            greedy(&c)
        };
        let planning_s = if external_plan.is_none() {
            Some(planning_start.elapsed().as_secs_f64())
        } else {
            None
        };
        json!({"family":family,"seed":seed,"evaluation":evaluate(&c,&p,delay)?,"planning_wall_s":planning_s})
    };
    let elapsed = start.elapsed().as_secs_f64();
    let mut result = result;
    if !args.iter().any(|a| a == "--emit-input") {
        result
            .as_object_mut()
            .unwrap()
            .insert("computation_wall_s".into(), json!(elapsed));
    }
    if let Some(path) = value("--out") {
        fs::write(path, serde_json::to_vec_pretty(&result).unwrap()).map_err(|e| e.to_string())?;
    }
    println!(
        "{}",
        json!({"family":family,"seed":seed,"computation_wall_s":elapsed,
        "earned":result.pointer("/evaluation/earned").or_else(||result.pointer("/best/earned")),"candidate_count":result.get("candidate_count")})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_action_class_has_397_valid_routes() {
        let all = plans();
        assert_eq!(all.len(), 397);
        for p in all {
            validate(&p).unwrap();
        }
    }
    #[test]
    fn paired_cases_change_only_deadlines() {
        let a = case(0, false);
        let b = case(0, true);
        assert_eq!(
            serde_json::to_value(&a.scenario).unwrap(),
            serde_json::to_value(&b.scenario).unwrap()
        );
        for (i, j) in a.tasks.iter().zip(&b.tasks) {
            assert_eq!((i.id, i.goal, i.reward), (j.id, j.goal, j.reward));
            assert_eq!(i.deadline_tick - j.deadline_tick, 300);
        }
    }
    #[test]
    fn duplicate_assignment_is_rejected() {
        let mut p = empty_plan();
        p.routes[0].task_ids.push(1);
        p.routes[1].task_ids.push(1);
        assert!(validate(&p).is_err());
    }
    #[test]
    fn public_input_contains_no_enemy_entities() {
        let input = public_input(&case(0, true));
        assert_eq!(input["observation"]["units"].as_array().unwrap().len(), 3);
        assert!(input["observation"]["contacts"]
            .as_array()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn public_reconstruction_preserves_navigation_scores_without_enemy_truth() {
        let judge = case(0, true);
        let public = from_public(&public_input(&judge)).unwrap();
        assert_eq!(public.start_tick, PUBLICATION);
        assert_eq!(public.scenario.units.len(), 3);
        assert!(public.scenario.units.iter().all(|u| u.side == Side::Blue));
        let p = greedy(&judge);
        for delay in [0, 60, 180] {
            let a = evaluate(&judge, &p, delay).unwrap();
            let b = evaluate(&public, &p, delay).unwrap();
            assert_eq!(a.earned, b.earned);
            assert_eq!(a.reached, b.reached);
        }
    }
}

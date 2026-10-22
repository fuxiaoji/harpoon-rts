//! Independent research runner. Uses the production core, never a player server.
//! Scripted latency is an instrumentation control, not measured LLM reasoning.
#[path = "realtime_research/runner.rs"]
mod runner;

use runner::{replay, run, Clock, Config, Policy, Template};
use std::{fs, path::PathBuf};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |key: &str| -> Option<String> {
        args.windows(2).find(|w| w[0] == key).map(|w| w[1].clone())
    };
    if let Some(path) = value("--replay") {
        let result: runner::Run =
            serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        replay(&result)?;
        println!(
            "{}",
            serde_json::json!({"replay_verified": true, "world_hash": result.world_hash})
        );
        return Ok(());
    }
    let case = value("--case").unwrap_or_else(|| "d0".into());
    if !["d0", "d1", "d2"].contains(&case.as_str()) {
        return Err("--case must be d0, d1 or d2".into());
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../research/realtime/scenarios/{case}.json"));
    let template: Template =
        serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let policy = Policy::parse(&value("--policy").unwrap_or_else(|| "event".into()))?;
    let latency_s = value("--latency-s")
        .unwrap_or_else(|| "5".into())
        .parse::<i64>()
        .map_err(|e| e.to_string())?;
    let seed = value("--seed")
        .unwrap_or_else(|| "0".into())
        .parse::<u64>()
        .map_err(|e| e.to_string())?;
    let side = harpoon_core::Side::parse(&value("--side").unwrap_or_else(|| "blue".into()))
        .ok_or("--side must be blue or red")?;
    let clock = match value("--clock").as_deref().unwrap_or("injected") {
        "injected" => Clock::Injected,
        "wall" => Clock::Wall {
            tick_ms: value("--tick-ms")
                .unwrap_or_else(|| "2".into())
                .parse::<u64>()
                .map_err(|e| e.to_string())?,
        },
        _ => return Err("--clock must be injected or wall".into()),
    };
    let result = run(
        &template,
        Config {
            policy,
            latency_s,
            seed,
            side,
            mirror: args.iter().any(|a| a == "--mirror"),
            clock,
        },
    )?;
    replay(&result)?;
    if let Some(path) = value("--out") {
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(
            path,
            serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    println!(
        "{}",
        serde_json::to_string(&result.summary()).map_err(|e| e.to_string())?
    );
    Ok(())
}

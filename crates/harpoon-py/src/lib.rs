//! PyO3 bindings for `harpoon-core`.
//!
//! The public surface is deliberately JSON-in/JSON-out: [`HarpoonEnv`] never
//! converts commands or observations field-by-field. Python deserializes the
//! returned strings with `json.loads`, so the Rust API and the wire/session
//! API used by the server stay in lockstep (same serde types, same schema).
//!
//! One call to [`HarpoonEnv::step`] advances exactly one authoritative tick.
//! Only commands whose `unit` is owned by the configured side are forwarded to
//! the engine; everything else is dropped rather than failing the whole step.

use harpoon_core::commands::Command;
use harpoon_core::db::Database;
use harpoon_core::observe::{observe, Observation};
use harpoon_core::scenario::{load_scenario, scenario_from_json, Scenario};
use harpoon_core::sim;
use harpoon_core::types::{EntityId, Side};
use harpoon_core::world::World;
use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use serde::Serialize;

/// Upper bound passed to `observe`. The engine's event ring buffer is 4096
/// entries, so a full-history projection is never truncated by this.
const MAX_EVENTS: usize = 4096;

/// Shape returned by `step`: `{"tick": .., "over": .., "observation": {..}}`.
#[derive(Serialize)]
struct StepResult<'a> {
    tick: i64,
    over: bool,
    observation: &'a Observation,
}

/// A single-side view of one running scenario.
///
/// `event_cursor` makes `step()` return only events that became visible since
/// the previous call (reset / last step). `observation()` always returns the
/// full visible event history, matching the server's `GET /api/state?since=0`.
#[pyclass(module = "harpoon_py")]
pub struct HarpoonEnv {
    /// Immutable scenario source; `seed` is overridden per episode.
    scenario: Scenario,
    side: Side,
    /// Seed used for the current episode (and reused by `reset(None)`).
    seed: u64,
    world: World,
    /// Last event sequence returned by `step()`; reset by `reset()`.
    event_cursor: u64,
}

#[pymethods]
impl HarpoonEnv {
    /// Load a scenario JSON file and build a world for `side` ("Blue"/"Red").
    ///
    /// A provided `seed` overrides the scenario's own seed. Without one, the
    /// seed embedded in the scenario file is authoritative.
    #[new]
    #[pyo3(signature = (scenario_json_path, side, seed=None))]
    fn new(scenario_json_path: String, side: String, seed: Option<u64>) -> PyResult<Self> {
        let side = parse_side(&side)?;

        let text = std::fs::read_to_string(&scenario_json_path).map_err(|e| {
            PyIOError::new_err(format!(
                "failed to read scenario {scenario_json_path:?}: {e}"
            ))
        })?;
        let mut scenario = scenario_from_json(&text).map_err(|e| {
            PyValueError::new_err(format!(
                "invalid scenario JSON in {scenario_json_path:?}: {e}"
            ))
        })?;

        let seed = seed.unwrap_or(scenario.seed);
        scenario.seed = seed;
        let world = build_world(&scenario)?;

        Ok(Self { scenario, side, seed, world, event_cursor: 0 })
    }

    /// Rebuild the world from the original scenario and return an observation.
    ///
    /// With `seed=None` the seed of the current episode is reused, so repeated
    /// `reset()` calls are deterministic. Pass a seed to start a new episode
    /// with that seed.
    #[pyo3(signature = (seed=None))]
    fn reset(&mut self, seed: Option<u64>) -> PyResult<String> {
        if let Some(seed) = seed {
            self.seed = seed;
        }
        self.scenario.seed = self.seed;
        self.world = build_world(&self.scenario)?;

        let observation = observe(&self.world, self.side, 0, MAX_EVENTS);
        advance_cursor(&mut self.event_cursor, &observation);
        to_json(&observation)
    }

    /// Advance one tick (one simulated second).
    ///
    /// `commands_json` must be a JSON array of `harpoon_core::commands::Command`
    /// objects. Commands bound to units not owned by this environment's side
    /// (including unknown unit ids) are filtered out. The command list is
    /// applied once for this tick, like an RTS standing-order burst.
    fn step(&mut self, commands_json: String) -> PyResult<String> {
        let commands: Vec<Command> = serde_json::from_str(&commands_json).map_err(|e| {
            PyValueError::new_err(format!(
                "commands_json must be a JSON array of Command objects: {e}"
            ))
        })?;

        if !self.world.over {
            let owned: Vec<(Side, Command)> = commands
                .into_iter()
                .filter(|cmd| self.owns(command_unit(cmd)))
                .map(|cmd| (self.side, cmd))
                .collect();
            sim::tick(&mut self.world, &owned);
        }

        self.step_result()
    }

    /// Current observation as JSON (full event history for this side).
    fn observation(&self) -> PyResult<String> {
        let observation = observe(&self.world, self.side, 0, MAX_EVENTS);
        to_json(&observation)
    }

    /// Authoritative tick counter.
    #[getter]
    fn tick(&self) -> i64 {
        self.world.tick
    }

    /// Whether the scenario has reached a terminal result.
    #[getter]
    fn over(&self) -> bool {
        self.world.over
    }

    /// Side controlled by this environment ("Blue" or "Red").
    #[getter]
    fn side(&self) -> &'static str {
        self.side.name()
    }

    fn __repr__(&self) -> String {
        format!(
            "HarpoonEnv(side={}, scenario={:?}, tick={}, over={})",
            self.side.name(),
            self.world.scenario_id,
            self.world.tick,
            self.world.over
        )
    }
}

impl HarpoonEnv {
    /// True when `unit` exists and belongs to this environment's side.
    fn owns(&self, unit: EntityId) -> bool {
        self.world
            .entity(unit)
            .is_some_and(|entity| entity.side == self.side)
    }

    /// Serialize this tick's tail of events plus current world status.
    fn step_result(&mut self) -> PyResult<String> {
        let observation = observe(&self.world, self.side, self.event_cursor, MAX_EVENTS);
        advance_cursor(&mut self.event_cursor, &observation);
        let payload = StepResult {
            tick: self.world.tick,
            over: self.world.over,
            observation: &observation,
        };
        to_json(&payload)
    }
}

/// Load a scenario with the built-in database, mapping failures to ValueError.
fn build_world(scenario: &Scenario) -> PyResult<World> {
    load_scenario(scenario.clone(), Database::built_in())
        .map_err(|e| PyValueError::new_err(format!("failed to load scenario: {e}")))
}

fn parse_side(side: &str) -> PyResult<Side> {
    Side::parse(side)
        .ok_or_else(|| PyValueError::new_err(format!("side must be Blue or Red, got {side:?}")))
}

/// Move the cursor to the newest visible event.
///
/// An empty tail must not move the cursor backwards, or the next step would
/// replay older events.
fn advance_cursor(cursor: &mut u64, observation: &Observation) {
    if let Some(event) = observation.events.last() {
        *cursor = event.seq;
    }
}

/// Every `Command` carries a `unit`, which determines command ownership.
fn command_unit(command: &Command) -> EntityId {
    match command {
        Command::SetCourse { unit, .. }
        | Command::SetSpeed { unit, .. }
        | Command::SetDepth { unit, .. }
        | Command::SetAltitude { unit, .. }
        | Command::Goto { unit, .. }
        | Command::Stop { unit }
        | Command::Follow { unit, .. }
        | Command::ClearStation { unit }
        | Command::SetSonar { unit, .. }
        | Command::SetWeaponsFree { unit, .. }
        | Command::SetAutoChaff { unit, .. }
        | Command::AssignTarget { unit, .. }
        | Command::ClearTarget { unit }
        | Command::Fire { unit, .. }
        | Command::DeployChaff { unit }
        | Command::LaunchAircraft { unit, .. }
        | Command::RecoverAircraft { unit } => *unit,
    }
}

fn to_json<T: Serialize>(value: &T) -> PyResult<String> {
    serde_json::to_string(value)
        .map_err(|e| PyRuntimeError::new_err(format!("failed to serialize JSON: {e}")))
}

#[pymodule]
fn harpoon_py(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<HarpoonEnv>()?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

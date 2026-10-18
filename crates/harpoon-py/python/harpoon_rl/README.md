# harpoon_rl — Gym-style RL environment

Python bindings for the deterministic `harpoon-core` naval wargame engine.
The compiled extension is `harpoon_py`; `harpoon_rl.HarpoonGymEnv` adds a
Gymnasium-shaped wrapper (no `gymnasium` dependency required).

## Build / install

`maturin develop` must run inside a virtual environment.

```sh
cd /path/to/repo
python3 -m venv .venv
source .venv/bin/activate
pip install maturin
maturin develop --manifest-path crates/harpoon-py/Cargo.toml
```

This installs both the `harpoon_py` extension and the `harpoon_rl` Python
package (from `python-source = "python"`). Maturin's mixed layout requires a
Python package matching the extension's import name, so the compiled module
lands at `python/harpoon_py/harpoon_py*.so` and `python/harpoon_py/__init__.py`
re-exports it (`import harpoon_py; harpoon_py.HarpoonEnv`). The bindings use
PyO3 0.30.1, which supports CPython 3.14 (the host interpreter), with
`extension-module` enabled; no `abi3` fallback was needed.

## Usage

Run from the repository root (or pass an absolute scenario path):

```python
from harpoon_rl import HarpoonGymEnv

env = HarpoonGymEnv("scenarios/gunnery_dual.json", side="Blue", seed=7)
observation, info = env.reset()          # observation is a dict

action = [
    {"cmd": "set_course", "unit": 1, "course_deg": 90},
    {"cmd": "set_speed", "unit": 1, "speed_kts": 25},
]
observation, reward, terminated, truncated, info = env.step(action)
print(observation["tick"], observation["units"][0]["pos"])
print(info["events"])                    # events since reset / previous step
env.render()                             # text summary
```

Command objects and the observation schema are the same JSON used by the HTTP
server and documented in the engine (`cmd` tag, snake_case variants, numeric
unit/contact ids). Actions whose `unit` is not owned by this environment's side
are filtered out by the binding.

`reset(seed)` accepts an optional seed; `reset()` reuses the current episode
seed. `step` returns the Gymnasium 5-tuple; `terminated` mirrors the engine's
`over` flag and `truncated` is always `False` (the engine ends episodes at its
scenario time limit).

### Reward

Reward semantics were not fixed by the engine, so the wrapper ships a small
deterministic default: `+1` for a visible loss of an opposing unit/base
(`sunk`, `ditched`, `air_shot_down`, `base_destroyed`), `-1` for one of your
own, plus a one-time `+10` / `-10` win/loss bonus at the terminal tick. Pass
`reward_fn=lambda observation, events: ...` to replace it entirely.

### Events

`step` observations and `info["events"]` contain only events that became
visible since the previous `step`/`reset`; `env.current_observation()` returns
the full visible event history for the configured side. The engine remains
double-blind: enemy units are never present in `observation["units"]`, only as
estimated `contacts`.

## Direct extension use

```python
import json
import harpoon_py

env = harpoon_py.HarpoonEnv("scenarios/gunnery_dual.json", "Blue", 7)
observation = json.loads(env.reset())
result = json.loads(env.step(json.dumps([{"cmd": "stop", "unit": 1}])))
assert result["observation"]["tick"] == 1
```

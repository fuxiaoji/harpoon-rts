# harpoon-mcp

An MCP (Model Context Protocol) **stdio** server that exposes the deterministic
harpoon engine to any MCP-capable LLM client. The JSON-RPC 2.0 framing is
written by hand on `serde_json`; no MCP SDK is used. The engine runs in-process
(`harpoon-core`), so a session needs no network service.

## Build

From the repository root:

```sh
source ~/.cargo/env
cargo build -p harpoon-mcp --release
```

## Register with an MCP client

Add this to the client's MCP configuration (for example Claude Desktop's
`claude_desktop_config.json`, or any client using the `mcpServers` format):

```json
{
  "mcpServers": {
    "harpoon": {
      "command": "cargo",
      "args": ["run", "-p", "harpoon-mcp", "--release"],
      "cwd": "/Users/Zhuanz1/Desktop/鱼叉"
    }
  }
}
```

`cwd` must be the repository root because scenarios are resolved as
`./scenarios/*.json` relative to the server's working directory. Alternatively,
build first and point `command` at `target/release/harpoon-mcp` directly.

The server reads one JSON-RPC request per line from stdin and writes one
response per line to stdout. stdout is reserved for protocol messages;
diagnostics go to stderr. `initialize` advertises protocol version
`2024-11-05` with the `tools` capability.

## Tools

| Tool | Arguments | Purpose |
| --- | --- | --- |
| `list_scenarios` | none | List `./scenarios/*.json` as `[{id,name,file}]` |
| `new_game` | `scenario`, `seed?` | Load a scenario and start/replace the session |
| `get_state` | `side`, `since?`, `max_events?` | Double-blind `Observation` for one side |
| `send_command` | `side`, `command` | Queue one command for the next `step` |
| `set_time` | `speed` | Advisory 0..512 time multiplier (0 pauses) |
| `step` | `ticks` | Apply queued commands on the first tick, then tick |
| `get_events` | `side`, `since?` | Event log visible to that side |
| `run_selfplay` | `scenario`, `seed?`, `max_ticks?` | Independent deterministic AI-vs-AI match summary |

Tool results are JSON encoded as a string inside the standard MCP
`content: [{type: "text", ...}]` envelope. Tool-level failures set
`isError: true`; unknown JSON-RPC methods return `-32601`.

### Commands

`send_command.side` is `Blue` or `Red`. The `command` object uses the engine's
serde schema (`cmd`, snake_case); examples:

```json
{"cmd":"set_course","unit":1,"course_deg":90}
{"cmd":"set_speed","unit":1,"speed_kts":25}
{"cmd":"goto","unit":1,"waypoints":[[100,50],[80,60]]}
{"cmd":"assign_target","unit":1,"contact":3}
{"cmd":"fire","unit":1,"contact":3,"weapon":"gun_5in","rounds":1}
{"cmd":"launch_aircraft","unit":1,"class":"fighter","count":2,
 "mission":{"mission":"cap","station":[10,10],"radius_nm":25}}
```

Coordinates are nautical miles on a flat plane (+x east, +y north).

## Time model

The server is deliberately **step-driven** and never advances the simulation on
its own. `set_time` records the desired multiplier so the host can schedule
calls; the host calls `step` explicitly:

```
set_time {speed: 60}   -> advisory: 60 simulated seconds per wall second
every 100 ms: step {ticks: 6}
```

Commands submitted with `send_command` are buffered and applied once, on the
first tick of the next `step`. Entity orders (course, speed, target, ...) then
persist inside the engine until changed, matching the server's real-time mode.

## Double-blind guarantee

`get_state` and `get_events` call `harpoon_core::observe` / `World::events_for`,
so clients only see their own units plus contacts with estimated positions and
qualities. Referee-only truth and the other side's events never leave the
process.

## Example session (manually over stdio)

```sh
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_scenarios","arguments":{}}}' \
  '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"new_game","arguments":{"scenario":"gunnery_dual","seed":123}}}' \
  '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"step","arguments":{"ticks":60}}}' \
  '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"get_state","arguments":{"side":"Blue"}}}' \
  '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"run_selfplay","arguments":{"scenario":"gunnery_dual","seed":123,"max_ticks":1200}}}' \
  | cargo run -p harpoon-mcp 2>/dev/null
```

## Selfplay

`run_selfplay` builds a fresh world from the scenario (it does not touch the
live session world) and drives both sides with a deterministic heuristic:
weapons-free posture for every unit so the engine's fire-control logic engages,
plus deliberate submarine torpedo attacks against tracked surface contacts. It
returns `ticks_run`, `over`, `result`, VP, per-event counts, surviving entity
count and a stable state digest; the same scenario + seed + tick cap reproduces
the same digest.

---
title: Where the continuous host stands
subtitle: Built and tested offline, run live only in bounded windows, not running at check time
kicker: Rung · rung-host · status
meta: master af62b7b, workspace 0.2.3. Sources - docs/rung-host*.md, rung-host/live/runs/.
footer: rung-host-status-v1
---

**Built.** Host loop, free-time kernel, cache pack and durable record (#153); engine split out as `rung-agent-core` (#152); ladder with listing filter and probes (#177, #214); ACP over stdio and HTTP (#178, #182); `run --config` (#196); shadow decision desk (#199); status report (#241); 24 h simulated soak (#238); opt-in idle rule (#247); named ladder default (#250); decision model switched to `microsoft/microsoft-decision-1` (#260, #271).

**Running: nothing.** No `rung-host` process, no rung unit under `~/.config/systemd/user`, no rung port open. Newest live record: `rung-host/live/runs/2026-10-06-named-ladder/`, started by hand with `live.sh`.

**Measured.** Offline, 12 gates pass at $0 (`docs/rung-host-results.md`), including 50 `kill -9` restarts losing nothing. Live, on free models: a 26 min first run failed owner latency and found four defects; a 2 h run had 154 turns, 42% failed upstream, cache 0.95; a named-ladder pair compared the idle rule off and on. No crash; agent spend $0.

**Not built or not proven.** Worker delegation (names reserved only). Idle rule: barely exercised, no verdict. Decision-model disagreement audit: 6 real fixtures replayed (26 of 30 agree), the 50-item shadow sheet has no verdicts. Shadow week: never run; longest window 2 h. Decision mode `decide` never run live. Issue #259 still open.

**Observe and control, as built.** No metrics endpoint, no MCP, no instance registry.

| Surface | Address and auth | Shape |
|---|---|---|
| ACP | stdio `--acp` (process owner); HTTP `acp.http`, bearer token per role from env vars | prompts; `_rung/status`, `report`, `calendar`, `stimulus`, `stop`, `release` |
| Files | `<state>/record/`, `inbox`, `stop_file`, `desk.kill_file`; file permissions | NDJSON record (the only log); `*.msg` stimuli; a file halts the host or mutes the decision model |
| Process | SIGTERM, `sd_notify` | clean stop; ready and watchdog |

**More than one instance?** By config only: separate `state`, `workspace`, `inbox`, stop file and ACP port. No lock guards a shared state dir; nothing lists instances.

# AGENTS.md — rung-agent-core

The agent as a library: catalog, sessions, the turn engine, MCP, memory hooks, the turn check, ACP and OAuth. `rung-agent` is a thin shell over it and `rung-host` imports it.
Repo charter: [`../AGENTS.md`](../AGENTS.md).

**Status.** Product library. `publish = false`.

**Read.**
- The CLI, ACP and configuration notes: [`../rung-agent/AGENTS.md`](../rung-agent/AGENTS.md).
- OAuth contract: [`docs/rung-agent-oauth.md`](../docs/rung-agent-oauth.md); prompt cache: [`docs/rung-agent-cache.md`](../docs/rung-agent-cache.md).

**Check.** `cargo test -p rung-agent-core --locked`

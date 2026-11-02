# AGENTS.md — rung-gateway

The HTTP gateway in front of hosts: one listener for the UI, any number of instances behind it, the instance keys held here, a read-only token role.
Repo charter: [`../AGENTS.md`](../AGENTS.md).

**Status.** Product binary and library. `publish = false`.

**Read.**
- Door shapes and configuration: [`docs/rung-host-api.md`](../docs/rung-host-api.md). The page it serves is `ui/`.

**Check.** `cargo test -p rung-gateway --locked`

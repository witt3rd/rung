# AGENTS.md — rung-gateway

The HTTP gateway in front of hosts: one listener for the UI, any number of instances behind it, the instance keys held here, a read-only token role.
Repo charter: [`../AGENTS.md`](../AGENTS.md).

**Status.** Product binary and library. `publish = false`.

**Read.**
- Door shapes and configuration: [`docs/reference/rung-gateway.md`](../docs/reference/rung-gateway.md). The page it serves is `ui/`.

**Check.** `cargo test -p rung-gateway --locked`

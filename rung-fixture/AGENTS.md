# AGENTS.md — rung-fixture

Cross-crate consumption tests: a ladder declared in one crate and consumed from `tests/`, which Rust compiles as a separate crate, pinning what a crate boundary does and does not carry.
Repo charter: [`../AGENTS.md`](../AGENTS.md).

**Status.** Test-only. `publish = false`.

**Read.**
- The boundary it pins is the proposition `cross-crate-provenance` in [`docs/rung-props.md`](../docs/rung-props.md). `tests/ui/` holds the `trybuild` refusals.

**Check.** `cargo test -p rung-fixture --locked`
